# Release Management

This repository uses automated semantic releases from the `master` branch.

## What is automated

- Semantic versioning from Conventional Commits
- `CHANGELOG.md` updates on each release
- Git tag creation in `vX.Y.Z` format
- GitHub Release creation with generated notes
- Contract artifact publishing (`.wasm`, checksums, release notes)

## Commit conventions

Use Conventional Commits in merged PRs:

- `feat:` creates a **minor** release
- `fix:` creates a **patch** release
- `perf:` creates a **patch** release
- `!` or `BREAKING CHANGE:` creates a **major** release
- `docs:`, `chore:`, `test:` do not create a release by default

Examples:

- `feat: add exam history pagination`
- `fix: prevent duplicate patient registration`
- `feat!: change record schema for normalized diagnostics`

## Workflow behavior

The release workflow is defined in `.github/workflows/release.yml`.

On each push to `master`, it:

1. Builds optimized contract `.wasm` artifacts.
2. Produces `dist/SHA256SUMS.txt`.
3. Calculates the next semantic version from commit history.
4. Updates `CHANGELOG.md`.
5. Updates `Cargo.toml` workspace version.
6. Creates/pushes a `vX.Y.Z` tag.
7. Publishes a GitHub Release and uploads artifacts:
   - `dist/*.wasm`
   - `dist/SHA256SUMS.txt`
   - `dist/RELEASE_NOTES.md`

## Scripts used by release automation

- `scripts/build_release_artifacts.sh`: builds contract wasm files and checksums.
- `scripts/prepare_release_version.sh`: updates workspace version in `Cargo.toml`.
- `scripts/generate_release_notes.sh`: creates artifact-focused release notes.

## Manual release run

You can run the workflow manually from GitHub Actions using `workflow_dispatch`.

To dry-run semantic release logic locally:

```bash
GITHUB_TOKEN=dummy npx -y \
  -p semantic-release \
  -p @semantic-release/changelog \
  -p @semantic-release/commit-analyzer \
  -p @semantic-release/exec \
  -p @semantic-release/git \
  -p @semantic-release/github \
  -p @semantic-release/release-notes-generator \
  -p conventional-changelog-conventionalcommits \
  semantic-release --dry-run
```
