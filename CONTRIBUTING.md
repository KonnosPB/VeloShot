# Contributing to VeloShot

Thank you for contributing to **VeloShot**!

## Branch & Pull Request Workflow

- Always open Pull Requests against **`develop`** (not `main`). Releases are merged from `develop` into `main`.
- Keep commits focused and free of machine-specific paths or secrets.

## Prerequisites (Ubuntu)

```bash
sudo apt update && sudo apt install -y libgtk-4-dev libadwaita-1-dev pkg-config libdbus-1-dev
```

## Checks Before Submitting

Please ensure all CI checks pass locally before opening a Pull Request:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```
