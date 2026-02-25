# First Contribution Walkthrough

This walkthrough takes you from fork to merged PR.

## 1) Fork and Clone

```bash
git clone https://github.com/YOUR_USERNAME/Teye-Contracts.git
cd Teye-Contracts
```

## 2) Setup and Verify

Run the setup script and verify tests pass:

```bash
./setup.sh
make test
```

## 3) Find a Good First Issue

- Look for issues labeled `good first issue` or `help wanted`.
- Comment on the issue so maintainers know you are working on it.

## 4) Create a Feature Branch

Branch naming conventions:

- `feature/<issue-id>-short-title`
- `fix/<issue-id>-short-title`
- `docs/<issue-id>-short-title`

```bash
git checkout -b feature/123-add-vision-export
```

## 5) Make Changes with Code Style

- Format: `cargo fmt`
- Lint: `cargo clippy`
- Keep changes focused and well-scoped.

## 6) Write Tests

Add or update tests that cover your change. Use `cargo test --all` and add targeted tests if needed.

## 7) Run the Local CI Pipeline

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo deny check
cargo audit
```

## 8) Commit with Conventional Commits

```bash
git commit -m "feat: add export support for records (closes #123)"
```

## 9) Push and Open a PR

```bash
git push origin feature/123-add-vision-export
```

Fill out the PR template, link the issue, and include testing output.

## 10) Respond to Review

- Address requested changes quickly.
- Push follow-up commits to the same branch.

## 11) Celebrate

Once merged, update any follow-up docs or issues if needed.
