# Fuzz Testing Guide

## Introduction
Fuzz testing (or fuzzing) is an automated software testing technique that involves providing invalid, unexpected, or random data as inputs to a computer program. We use it to ensure structural integrity of Stellar Teye Contracts and discover hidden edge cases or integer overflows.

The `cargo-fuzz` and `libFuzzer` infrastructure is integrated into our contracts. For comprehensive fuzzing strategy and corpus management, see the [Testing Strategy Guide](testing-strategy.md#fuzz-testing-deep-dive).

## Setup
To run fuzzers locally, you need nightly toolchain and `cargo-fuzz` installed:
```bash
cargo install cargo-fuzz
rustup default nightly
```

## Running Fuzzers
The workspace has a comprehensive `fuzz` setup at the root directory targeting smart contracts. To run a specific fuzzer campaign:

```bash
cd fuzz
cargo +nightly fuzz run vision_records
```

### Available Targets
- **`vision_records`**: Explores `VisionRecordsContract`, looking at data parsing errors, hashing bugs, or access control panic flows under unpredictable conditions.
- **`staking`**: Aggressively stakes and unstakes varying parameters, ensuring there are no arithmetic panics unhandled by contract.
- **`audit`**: Tests audit logging and compliance features under random input conditions.

## Corpus Management

### Corpus Directory Structure
```
fuzz/
├── corpus/
│   ├── vision_records/
│   │   ├── patient_registration/
│   │   ├── record_access/
│   │   └── emergency_scenarios/
│   ├── staking/
│   └── audit/
└── artifacts/
    ├── crashes/
    └── hangs/
```

### Managing Seed Files
```bash
# Add interesting input to corpus
cp interesting_input.json fuzz/corpus/vision_records/

# Minimize corpus
cargo +nightly fuzz cmin vision_records

# Merge corpora from multiple runs
cargo +nightly fuzz merge vision_records corpus1/ corpus2/
```

## Sanitizer Integration

### Address Sanitizer
```bash
RUSTFLAGS="-Z sanitizer=address" cargo +nightly fuzz run vision_records
```

### Memory Sanitizer
```bash
RUSTFLAGS="-Z sanitizer=memory" cargo +nightly fuzz run vision_records
```

### Undefined Behavior Sanitizer
```bash
RUSTFLAGS="-Z sanitizer=undefined" cargo +nightly fuzz run vision_records
```

## Investigating Fuzzer-Found Bugs

### Reproducing Crashes
```bash
# Replay a specific crash
cargo +nightly fuzz replay vision_records artifacts/crashes/crash-xxxxx

# Minimize the crashing input
cargo +nightly fuzz tmin vision_records artifacts/crashes/crash-xxxxx

# Generate stack trace
RUST_BACKTRACE=1 cargo +nightly fuzz replay vision_records artifacts/crashes/crash-xxxxx
```

### Common Bug Patterns
1. **Buffer Overflows**: Check array bounds and string operations
2. **Integer Overflows**: Verify arithmetic operations
3. **State Inconsistencies**: Ensure contract state remains valid
4. **Access Control Bypass**: Test permission checks under all conditions

## CI Pipeline
Our GitHub Actions pipeline includes a dedicated job to run fuzz tests (`.github/workflows/fuzz.yml`). The CI runs these fuzz targets for a brief period on every pull request to catch obvious regressions. Extended fuzzing campaigns are continuously run on nightly builds to probe deep execution branches.

## Writing New Targets
1. Open `fuzz/fuzz_targets` directory.
2. Draft a new `no_main` Rust file incorporating `libfuzzer_sys::fuzz_target!`.
3. Register the target in `fuzz/Cargo.toml`.
4. Generate inputs utilizing the `arbitrary::Arbitrary` trait.
5. Set up proper test environment and contract initialization.
6. Handle panics gracefully to allow fuzzer continuation.

### Target Template
```rust
#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use soroban_sdk::{Env, Address};

#[derive(Arbitrary, Debug)]
pub enum FuzzAction {
    Action1 { param1: u8, param2: u32 },
    Action2 { data_len: usize },
}

fuzz_target!(|actions: Vec<FuzzAction>| {
    let env = Env::default();
    let contract_id = env.register(YourContract, ());
    let client = YourContractClient::new(&env, &contract_id);
    
    // Initialize contract
    client.initialize(&Address::generate(&env));
    
    // Execute fuzz actions
    for action in actions {
        execute_action(&env, &client, action);
    }
});
```

## Best Practices

1. **Start Small**: Begin with simple actions and gradually increase complexity
2. **Use Arbitrary**: Implement the `Arbitrary` trait for custom data types
3. **Handle Panics**: Use `catch_unwind` to prevent fuzzer crashes
4. **Monitor Coverage**: Use coverage-guided fuzzing for better results
5. **Regular Updates**: Keep fuzz targets updated with contract changes
6. **Corpus Sharing**: Share interesting inputs between team members

## Performance Tips

- Use `cargo fuzz run -- -jobs=N` to parallelize fuzzing
- Limit memory usage with `-max_len=N` parameter
- Set time limits with `-max_total_time=N` for CI runs
- Use `-dict=fuzz/dictionaries/contract.dict` for structured inputs

## References

- [cargo-fuzz Documentation](https://rust-fuzz.github.io/book/cargo-fuzz.html)
- [libFuzzer Documentation](https://llvm.org/docs/LibFuzzer.html)
- [Testing Strategy Guide](testing-strategy.md)
