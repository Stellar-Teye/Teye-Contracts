## 1.0.0 (2026-02-21)

### Features

* add automated release management workflow ([6856459](https://github.com/ChukwuemekaP1/Teye-Contracts/commit/6856459e18c1000b2121171b11112bb64f5aaa8e))
* implement event monitoring system ([d6f6c25](https://github.com/ChukwuemekaP1/Teye-Contracts/commit/d6f6c2546805cfb0b92f42fd455f08f7793b98b8))
* implement record versioning and history tracking ([5b6a6ea](https://github.com/ChukwuemekaP1/Teye-Contracts/commit/5b6a6ea5d2089510641b23e9b9ec8fa1347f0175))
* implement role based access control ([ef86cbf](https://github.com/ChukwuemekaP1/Teye-Contracts/commit/ef86cbf68b3db9a246901c06e146824d01d7c2cf))
* initial project scaffolding for Stellar-Teye vision care platform ([aca3c01](https://github.com/ChukwuemekaP1/Teye-Contracts/commit/aca3c015fa100b765343bac881beffd165deb2e2))
## [1.2.2](https://github.com/Stellar-Teye/Teye-Contracts/compare/v1.2.1...v1.2.2) (2026-02-22)

### Bug Fixes

* **rbac:** correctly persist user roles to storage on initialization and registration ([fbfae85](https://github.com/Stellar-Teye/Teye-Contracts/commit/fbfae859eb167d0f9be98412160a4c7bc3b7945e))
* **test:** remove corrupted inline unit test module from lib.rs ([2fa262b](https://github.com/Stellar-Teye/Teye-Contracts/commit/2fa262b8d5df8ce3ec0035643dcd894f245915e8))

## [1.1.0](https://github.com/precious-akpan/Teye-Contracts/compare/v1.0.0...v1.1.0) (2026-02-22)

### Features

* implement mutation testing and tests coverage for [#27](https://github.com/precious-akpan/Teye-Contracts/issues/27) ([ced0105](https://github.com/precious-akpan/Teye-Contracts/commit/ced010556c7c24a44e92b3609a3d4bd7c5096b01))
* **zk:** implement Issue [#43](https://github.com/precious-akpan/Teye-Contracts/issues/43) ZK Verifier integration ([eefb197](https://github.com/precious-akpan/Teye-Contracts/commit/eefb197f9f256c8788fb912dbafbd4ebadf8078b))

### Bug Fixes

* **ci:** Resolve clippy warning and missing toolchain input in mutation.yml ([d477d70](https://github.com/precious-akpan/Teye-Contracts/commit/d477d70c15e682e874f0cfb1951e1c4969e150ff))
* **test:** remove corrupted master test block in lib.rs ([f800c4b](https://github.com/precious-akpan/Teye-Contracts/commit/f800c4b3c979c3b30765f911d5ee6cd809c77b77))

## [1.2.1](https://github.com/Stellar-Teye/Teye-Contracts/compare/v1.2.0...v1.2.1) (2026-02-22)

### Bug Fixes

* **test:** remove corrupted master test block in lib.rs ([f800c4b](https://github.com/Stellar-Teye/Teye-Contracts/commit/f800c4b3c979c3b30765f911d5ee6cd809c77b77))

## [1.2.0](https://github.com/Stellar-Teye/Teye-Contracts/compare/v1.1.0...v1.2.0) (2026-02-22)

### Features

* implement mutation testing and tests coverage for [#27](https://github.com/Stellar-Teye/Teye-Contracts/issues/27) ([ced0105](https://github.com/Stellar-Teye/Teye-Contracts/commit/ced010556c7c24a44e92b3609a3d4bd7c5096b01))
* **zk:** implement Issue [#43](https://github.com/Stellar-Teye/Teye-Contracts/issues/43) ZK Verifier integration ([eefb197](https://github.com/Stellar-Teye/Teye-Contracts/commit/eefb197f9f256c8788fb912dbafbd4ebadf8078b))

### Bug Fixes

* **ci:** Resolve clippy warning and missing toolchain input in mutation.yml ([d477d70](https://github.com/Stellar-Teye/Teye-Contracts/commit/d477d70c15e682e874f0cfb1951e1c4969e150ff))

## [1.1.0](https://github.com/Stellar-Teye/Teye-Contracts/compare/v1.0.0...v1.1.0) (2026-02-21)

### Features

* implement record versioning and history tracking ([5b6a6ea](https://github.com/Stellar-Teye/Teye-Contracts/commit/5b6a6ea5d2089510641b23e9b9ec8fa1347f0175))

## 1.0.0 (2026-02-21)

### Features

* add automated release management workflow ([6856459](https://github.com/Stellar-Teye/Teye-Contracts/commit/6856459e18c1000b2121171b11112bb64f5aaa8e))
* implement event monitoring system ([d6f6c25](https://github.com/Stellar-Teye/Teye-Contracts/commit/d6f6c2546805cfb0b92f42fd455f08f7793b98b8))
* implement role based access control ([ef86cbf](https://github.com/Stellar-Teye/Teye-Contracts/commit/ef86cbf68b3db9a246901c06e146824d01d7c2cf))
* initial project scaffolding for Stellar-Teye vision care platform ([aca3c01](https://github.com/Stellar-Teye/Teye-Contracts/commit/aca3c015fa100b765343bac881beffd165deb2e2))

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial project scaffolding
- Vision records smart contract with basic functionality
- User registration and management
- Record creation and retrieval
- Access control system (grant/revoke access)
- Role-based access control (Patient, Optometrist, Ophthalmologist, Admin)
- Automated setup script
- CI/CD workflow with GitHub Actions
- Comprehensive documentation

### Changed
- N/A

### Deprecated
- N/A

### Removed
- N/A

### Fixed
- N/A

### Security
- N/A

## [0.1.0] - 2026-01-27

### Added
- Initial release
- Core vision records contract
- Basic RBAC implementation
- Project documentation
