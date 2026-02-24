# Code Quality Standards

## Overview

All code in this repository must adhere to the following standards:

1. **Test Coverage**
   - Minimum 80% coverage on all modules.
   - Coverage reports are generated automatically on PRs.

2. **Linting & Complexity**
   - JS/TS code is linted via ESLint.
   - Rust code uses Clippy with all warnings treated as errors.
   - Cyclomatic complexity should remain maintainable (measured with Plato for JS/TS).

3. **Documentation**
   - All public functions and modules must have docstrings.
   - Markdown documentation must pass markdownlint.

4. **PR Quality Checklist**
   - [ ] Code passes all tests
   - [ ] Coverage ≥ 80%
   - [ ] Linted and free of warnings
   - [ ] Documentation updated if functionality changed
   - [ ] All new logic reviewed and approved

5. **Automated Code Review**
   - GitHub Actions runs lint, tests, coverage, and complexity checks on every PR.
   - PRs failing checks cannot be merged.

## Enforcement

Failures in any automated checks will block merging to main. Contributors must fix violations before approval.
