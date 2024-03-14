# @hyperlane-xyz/helloworld

## 3.8.0

### Minor Changes

- 7d530fd4: Enabled verification of contracts as part of the deployment flow.

  - Solidity build artifact is now included as part of the `@hyperlane-xyz/core` package.
  - Updated the `HyperlaneDeployer` to perform contract verification immediately after deploying a contract. A default verifier is instantiated using the core build artifact.
  - Updated the `HyperlaneIsmFactory` to re-use the `HyperlaneDeployer` for deployment where possible.
  - Minor logging improvements throughout deployers.

### Patch Changes

- Updated dependencies [16cb5e19]
- Updated dependencies [90191f74]
- Updated dependencies [7d530fd4]
- Updated dependencies [aea9e143]
- Updated dependencies [b3a91546]
- Updated dependencies [912ced30]
- Updated dependencies [02e64c9f]
- Updated dependencies [d2c24967]
- Updated dependencies [3ff8eb3c]
  - @hyperlane-xyz/sdk@3.8.0
  - @hyperlane-xyz/core@3.8.0

## 3.7.0

### Patch Changes

- Updated dependencies [6f464eaed]
- Updated dependencies [87151c62b]
- Updated dependencies [ab17af5f7]
- Updated dependencies [7b40232af]
- Updated dependencies [54aeb6420]
  - @hyperlane-xyz/sdk@3.7.0
  - @hyperlane-xyz/core@3.7.0

## 3.6.2

### Patch Changes

- @hyperlane-xyz/core@3.6.2
- @hyperlane-xyz/sdk@3.6.2

## 3.6.1

### Patch Changes

- Updated dependencies [ae4476ad0]
- Updated dependencies [f3b7ddb69]
- Updated dependencies [e4e4f93fc]
  - @hyperlane-xyz/sdk@3.6.1
  - @hyperlane-xyz/core@3.6.1

## 3.6.0

### Patch Changes

- Updated dependencies [67a6d971e]
- Updated dependencies [612d4163a]
- Updated dependencies [0488ef31d]
- Updated dependencies [8d8ba3f7a]
  - @hyperlane-xyz/sdk@3.6.0
  - @hyperlane-xyz/core@3.6.0

## 3.5.1

### Patch Changes

- Updated dependencies [a04454d6d]
  - @hyperlane-xyz/sdk@3.5.1
  - @hyperlane-xyz/core@3.5.1

## 3.5.0

### Patch Changes

- Updated dependencies [655b6a0cd]
- Updated dependencies [08ba0d32b]
- Updated dependencies [f7d285e3a]
  - @hyperlane-xyz/sdk@3.5.0
  - @hyperlane-xyz/core@3.5.0

## 3.4.0

### Patch Changes

- Updated dependencies [7919417ec]
- Updated dependencies [fd4fc1898]
- Updated dependencies [e06fe0b32]
- Updated dependencies [b832e57ae]
- Updated dependencies [79c96d718]
  - @hyperlane-xyz/sdk@3.4.0
  - @hyperlane-xyz/core@3.4.0

## 3.3.0

### Patch Changes

- Updated dependencies [7e620c9df]
- Updated dependencies [350175581]
- Updated dependencies [9f2c7ce7c]
  - @hyperlane-xyz/sdk@3.3.0
  - @hyperlane-xyz/core@3.3.0

## 3.2.0

### Patch Changes

- Updated dependencies [df34198d4]
- Updated dependencies [df693708b]
  - @hyperlane-xyz/core@3.2.0
  - @hyperlane-xyz/sdk@3.2.0

## 3.1.10

### Patch Changes

- c9e0aedae: Improve client side StandardHookMetadata library interface
- Updated dependencies [c9e0aedae]
  - @hyperlane-xyz/core@3.1.10
  - @hyperlane-xyz/sdk@3.1.10
