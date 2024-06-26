use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::Arc;
use std::time::Duration;

use derive_new::new;
use futures_util::future::try_join_all;
use prometheus::{IntCounter, IntGauge, IntGaugeVec};
use tokio::spawn;
use tokio::sync::{
    mpsc::{self},
    Mutex,
};
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tracing::{debug, info_span, instrument, instrument::Instrumented, trace, Instrument};

use hyperlane_base::CoreMetrics;
use hyperlane_core::HyperlaneDomain;

use super::pending_operation::*;

/// Queue of generic operations that can be submitted to a destination chain.
/// Includes logic for maintaining queue metrics by the destination and `app_context` of an operation
#[derive(Debug, Clone, new)]
struct OpQueue {
    metrics: IntGaugeVec,
    queue_metrics_label: String,
    #[new(default)]
    queue: Arc<Mutex<BinaryHeap<Reverse<Box<DynPendingOperation>>>>>,
}

impl OpQueue {
    /// Push an element onto the queue and update metrics
    async fn push(&self, op: Box<DynPendingOperation>) {
        // increment the metric before pushing onto the queue, because we lose ownership afterwards
        self.get_operation_metric(&op).inc();

        self.queue.lock().await.push(Reverse(op));
    }

    /// Pop an element from the queue and update metrics
    async fn pop(&self) -> Option<Reverse<Box<DynPendingOperation>>> {
        let op = self.queue.lock().await.pop();
        op.map(|op| {
            // even if the metric is decremented here, the operation may fail to process and be re-added to the queue.
            // in those cases, the queue length will decrease to zero until the operation is re-added.
            self.get_operation_metric(&op.0).dec();
            op
        })
    }

    /// Get the metric associated with this operation
    fn get_operation_metric(&self, operation: &DynPendingOperation) -> IntGauge {
        let (destination, app_context) = operation.get_operation_labels();
        self.metrics
            .with_label_values(&[&destination, &self.queue_metrics_label, &app_context])
    }
}

/// SerialSubmitter accepts operations over a channel. It is responsible for
/// executing the right strategy to deliver those messages to the destination
/// chain. It is designed to be used in a scenario allowing only one
/// simultaneously in-flight submission, a consequence imposed by strictly
/// ordered nonces at the target chain combined with a hesitancy to
/// speculatively batch > 1 messages with a sequence of nonces, which entails
/// harder to manage error recovery, could lead to head of line blocking, etc.
///
/// The single transaction execution slot is (likely) a bottlenecked resource
/// under steady state traffic, so the SerialSubmitter implemented in this file
/// carefully schedules work items onto the constrained
/// resource (transaction execution slot) according to a policy that
/// incorporates both user-visible metrics and message operation readiness
/// checks.
///
/// Operations which failed processing due to a retriable error are also
/// retained within the SerialSubmitter, and will eventually be retried
/// according to our prioritization rule.
///
/// Finally, the SerialSubmitter ensures that message delivery is robust to
/// destination chain reorgs prior to committing delivery status to
/// HyperlaneRocksDB.
///
///
/// Objectives
/// ----------
///
/// A few primary objectives determine the structure of this scheduler:
///
/// 1. Progress for well-behaved applications should not be inhibited by
/// delivery of messages for which we have evidence of possible issues
/// (i.e., that we have already tried and failed to deliver them, and have
/// retained them for retry). So we should attempt processing operations
/// (num_retries=0) before ones that have been failing for a
/// while (num_retries>0)
///
/// 2. Operations should be executed in in-order, i.e. if op_a was sent on
/// source chain prior to op_b, and they're both destined for the same
/// destination chain and are otherwise eligible, we should try to deliver op_a
/// before op_b, all else equal. This is because we expect applications may
/// prefer this even if they do not strictly rely on it for correctness.
///
/// 3. Be [work-conserving](https://en.wikipedia.org/wiki/Work-conserving_scheduler) w.r.t.
/// the single execution slot, i.e. so long as there is at least one message
/// eligible for submission, we should be working on it within reason. This
/// must be balanced with the cost of making RPCs that will almost certainly
/// fail and potentially block new messages from being sent immediately.
#[derive(Debug, new)]
pub struct SerialSubmitter {
    /// Domain this submitter delivers to.
    domain: HyperlaneDomain,
    /// Receiver for new messages to submit.
    rx: mpsc::UnboundedReceiver<Box<DynPendingOperation>>,
    /// Metrics for serial submitter.
    metrics: SerialSubmitterMetrics,
}

impl SerialSubmitter {
    pub fn spawn(self) -> Instrumented<JoinHandle<()>> {
        let span = info_span!("SerialSubmitter", destination=%self.domain);
        spawn(async move { self.run().await }).instrument(span)
    }

    async fn run(self) {
        let Self {
            domain,
            metrics,
            rx: rx_prepare,
        } = self;
        let prepare_queue = OpQueue::new(
            metrics.submitter_queue_length.clone(),
            "prepare_queue".to_string(),
        );
        let confirm_queue = OpQueue::new(
            metrics.submitter_queue_length.clone(),
            "confirm_queue".to_string(),
        );

        // This is a channel because we want to only have a small number of messages
        // sitting ready to go at a time and this acts as a synchronization tool
        // to slow down the preparation of messages when the submitter gets
        // behind.
        let (tx_submit, rx_submit) = mpsc::channel(1);

        let tasks = [
            spawn(receive_task(
                domain.clone(),
                rx_prepare,
                prepare_queue.clone(),
            )),
            spawn(prepare_task(
                domain.clone(),
                prepare_queue.clone(),
                tx_submit,
                metrics.clone(),
            )),
            spawn(submit_task(
                domain.clone(),
                rx_submit,
                prepare_queue.clone(),
                confirm_queue.clone(),
                metrics.clone(),
            )),
            spawn(confirm_task(
                domain.clone(),
                prepare_queue,
                confirm_queue,
                metrics,
            )),
        ];

        if let Err(err) = try_join_all(tasks).await {
            tracing::error!(
                error=?err,
                ?domain,
                "SerialSubmitter task panicked for domain"
            );
        }
    }
}

#[instrument(skip_all, fields(%domain))]
async fn receive_task(
    domain: HyperlaneDomain,
    mut rx: mpsc::UnboundedReceiver<Box<DynPendingOperation>>,
    prepare_queue: OpQueue,
) {
    // Pull any messages sent to this submitter
    while let Some(op) = rx.recv().await {
        trace!(?op, "Received new operation");
        // make sure things are getting wired up correctly; if this works in testing it
        // should also be valid in production.
        debug_assert_eq!(*op.domain(), domain);
        prepare_queue.push(op).await;
    }
}

#[instrument(skip_all, fields(%domain))]
async fn prepare_task(
    domain: HyperlaneDomain,
    prepare_queue: OpQueue,
    tx_submit: mpsc::Sender<Box<DynPendingOperation>>,
    metrics: SerialSubmitterMetrics,
) {
    loop {
        // Pick the next message to try preparing.
        let next = prepare_queue.pop().await;

        let Some(Reverse(mut op)) = next else {
            // queue is empty so give some time before checking again to prevent burning CPU
            sleep(Duration::from_millis(200)).await;
            continue;
        };

        trace!(?op, "Preparing operation");
        debug_assert_eq!(*op.domain(), domain);

        match op.prepare().await {
            PendingOperationResult::Success => {
                debug!(?op, "Operation prepared");
                metrics.ops_prepared.inc();
                // this send will pause this task if the submitter is not ready to accept yet
                if let Err(err) = tx_submit.send(op).await {
                    tracing::error!(error=?err, "Failed to send prepared operation to submitter");
                }
            }
            PendingOperationResult::NotReady => {
                // none of the operations are ready yet, so wait for a little bit
                prepare_queue.push(op).await;
                sleep(Duration::from_millis(200)).await;
            }
            PendingOperationResult::Reprepare => {
                metrics.ops_failed.inc();
                prepare_queue.push(op).await;
            }
            PendingOperationResult::Drop => {
                metrics.ops_dropped.inc();
            }
        }
    }
}

#[instrument(skip_all, fields(%domain))]
async fn submit_task(
    domain: HyperlaneDomain,
    mut rx_submit: mpsc::Receiver<Box<DynPendingOperation>>,
    prepare_queue: OpQueue,
    confirm_queue: OpQueue,
    metrics: SerialSubmitterMetrics,
) {
    while let Some(mut op) = rx_submit.recv().await {
        trace!(?op, "Submitting operation");
        debug_assert_eq!(*op.domain(), domain);

        match op.submit().await {
            PendingOperationResult::Success => {
                debug!(?op, "Operation submitted");
                metrics.ops_submitted.inc();
                confirm_queue.push(op).await;
            }
            PendingOperationResult::NotReady => {
                panic!("Pending operation was prepared and therefore must be ready")
            }
            PendingOperationResult::Reprepare => {
                metrics.ops_failed.inc();
                prepare_queue.push(op).await;
            }
            PendingOperationResult::Drop => {
                metrics.ops_dropped.inc();
            }
        }
    }
}

#[instrument(skip_all, fields(%domain))]
async fn confirm_task(
    domain: HyperlaneDomain,
    prepare_queue: OpQueue,
    confirm_queue: OpQueue,
    metrics: SerialSubmitterMetrics,
) {
    loop {
        // Pick the next message to try confirming.
        let Some(Reverse(mut op)) = confirm_queue.pop().await else {
            sleep(Duration::from_secs(5)).await;
            continue;
        };

        trace!(?op, "Confirming operation");
        debug_assert_eq!(*op.domain(), domain);

        match op.confirm().await {
            PendingOperationResult::Success => {
                debug!(?op, "Operation confirmed");
                metrics.ops_confirmed.inc();
            }
            PendingOperationResult::NotReady => {
                // none of the operations are ready yet, so wait for a little bit
                confirm_queue.push(op).await;
                sleep(Duration::from_secs(5)).await;
            }
            PendingOperationResult::Reprepare => {
                metrics.ops_reorged.inc();
                prepare_queue.push(op).await;
            }
            PendingOperationResult::Drop => {
                metrics.ops_dropped.inc();
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct SerialSubmitterMetrics {
    submitter_queue_length: IntGaugeVec,
    ops_prepared: IntCounter,
    ops_submitted: IntCounter,
    ops_confirmed: IntCounter,
    ops_reorged: IntCounter,
    ops_failed: IntCounter,
    ops_dropped: IntCounter,
}

impl SerialSubmitterMetrics {
    pub fn new(metrics: &CoreMetrics, destination: &HyperlaneDomain) -> Self {
        let destination = destination.name();
        Self {
            submitter_queue_length: metrics.submitter_queue_length(),
            ops_prepared: metrics
                .operations_processed_count()
                .with_label_values(&["prepared", destination]),
            ops_submitted: metrics
                .operations_processed_count()
                .with_label_values(&["submitted", destination]),
            ops_confirmed: metrics
                .operations_processed_count()
                .with_label_values(&["confirmed", destination]),
            ops_reorged: metrics
                .operations_processed_count()
                .with_label_values(&["reorged", destination]),
            ops_failed: metrics
                .operations_processed_count()
                .with_label_values(&["failed", destination]),
            ops_dropped: metrics
                .operations_processed_count()
                .with_label_values(&["dropped", destination]),
        }
    }
}
