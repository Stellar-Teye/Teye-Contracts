# Development Environment Deep Dive

This guide covers Linux, macOS, and Windows (WSL2). It assumes you are working in the repository root.

## Supported Platforms

- Linux (Ubuntu 22.04+ recommended)
- macOS (Apple Silicon or Intel)
- Windows using WSL2 (Ubuntu recommended)

### Windows (WSL2) Notes

- Install WSL2 and Ubuntu, then do all work inside the WSL2 filesystem.
- Avoid building inside `/mnt/c` to prevent file watcher and permission issues.
- Use Windows Git only for GitHub Desktop if needed, but run builds in WSL2.

## Toolchain Setup

### Rust (Pinned)

The project expects Rust 1.78.0.

```bash
rustup toolchain install 1.78.0
rustup default 1.78.0
rustup component add rustfmt clippy rust-src
```

Verify:

```bash
rustc --version
cargo --version
```

### WASM Target

Soroban contracts compile to WASM for on-chain deployment.

```bash
rustup target add wasm32-unknown-unknown
```

Common failure signs:
- `error: cannot find target wasm32-unknown-unknown`
- `unknown target 'wasm32-unknown-unknown'`

### Soroban CLI

Install the Soroban CLI (v23.1.4).

```bash
cargo install --locked soroban-cli
soroban --version
```

If you need a specific version:

```bash
cargo install --locked soroban-cli --version 23.1.4
```

### Additional Cargo Tools

```bash
cargo install cargo-fuzz
cargo install cargo-deny
cargo install cargo-audit
```

### Pre-Commit Hooks

This repo ships a `.pre-commit-config.yaml`. Install and enable hooks:

```bash
pipx install pre-commit
pre-commit install
pre-commit run --all-files
```

## IDE Configuration

### VS Code

Recommended extensions:
- rust-lang.rust-analyzer
- vadimcn.vscode-lldb
- serayuzgur.crates
- tamasfe.even-better-toml
- github.vscode-github-actions

Recommended settings:
- Enable `rust-analyzer.checkOnSave.command` set to `clippy`.
- Enable `editor.formatOnSave`.
- Use `rust-analyzer.cargo.features` if you need feature flags.
- Set `files.watcherExclude` for `**/target/**` to reduce file watcher load.

### IntelliJ / CLion

- Install the Rust plugin.
- Enable `Run rustfmt on Save`.
- Enable `Clippy` inspections for Rust.
- Configure a toolchain that points to Rust 1.78.0.

## Docker Development

If you prefer containerized builds, use the repo Dockerfile to build a reproducible environment:

```bash
docker build -t teye-contracts:dev .
docker run --rm -it -v "$PWD:/workspace" teye-contracts:dev bash
```

From inside the container, run the normal build and test commands.

## Troubleshooting FAQ

1. `error: could not find Cargo.toml`
   - Ensure you are in the repo root before running cargo commands.
2. `rustc: unknown target wasm32-unknown-unknown`
   - Run `rustup target add wasm32-unknown-unknown`.
3. `linker `cc` not found` on Linux
   - Install build tools: `sudo apt-get install build-essential`.
4. `ld: library not found for -lssl` on macOS
   - Install OpenSSL via Homebrew and set `PKG_CONFIG_PATH`.
5. Soroban CLI version mismatch
   - Reinstall with `cargo install --locked soroban-cli --version 23.1.4`.
6. `soroban: command not found`
   - Ensure `~/.cargo/bin` is in your PATH and restart your shell.
7. `error: denied by policy` during `cargo deny check`
   - Update your advisory DB: `cargo deny fetch`.
8. `error: unable to get local issuer certificate`
   - Check corporate proxy settings or set `CARGO_HTTP_CAINFO`.
9. WSL2 file watcher slowness
   - Move the repo into the Linux filesystem and rerun.
10. Dockerfile not found
   - Verify you are in the repo root or confirm the file exists.
