# Fuzz Testing Guide

## Introduction
Fuzz testing (or fuzzing) is an automated software testing technique that involves providing invalid, unexpected, or random data as inputs to a computer program. We use it to ensure the structural integrity of the Stellar Teye Contracts and discover hidden edge cases or integer overflows.

The `cargo-fuzz` and `libFuzzer` infrastructure is integrated into our contracts.

## Setup
To run the fuzzers locally, you need the nightly toolchain and `cargo-fuzz` installed:
```bash
cargo install cargo-fuzz
rustup default nightly
```

## Running the Fuzzers
The workspace has a comprehensive `fuzz` setup at the root directory targeting the smart contracts. To run a specific fuzzer campaign (e.g. `vision_records_fuzz` or `staking_fuzz`), use:

```bash
cd fuzz
cargo +nightly run --bin vision_records_fuzz
```
*(Alternatively, you can run `cargo +nightly fuzz run vision_records_fuzz` if you prefer the `cargo-fuzz` CLI tool.)*

### Available Targets
- **`vision_records_fuzz`**: Explores the `VisionRecordsContract`, looking at data parsing errors, hashing bugs, or access control panic flows under unpredictable conditions.
- **`staking_fuzz`**: Aggressively stakes and unstakes varying parameters, ensuring there are no arithmetic panics unhandled by the contract.

## CI Pipeline
Our GitHub Actions pipeline includes a dedicated job to run fuzz tests (`.github/workflows/fuzz.yml`). The CI runs these fuzz targets for a brief period on every pull request to catch obvious regressions. Extended fuzzing campaigns are continuously run on nightly builds to probe deep execution branches.

## Writing New Targets
1. Open the `fuzz/fuzz_targets` directory.
2. Draft a new `no_main` Rust file incorporating `libfuzzer_sys::fuzz_target!`.
3. Register the new executable in `fuzz/Cargo.toml`.
4. Generate inputs utilizing the `arbitrary::Arbitrary` trait map.
