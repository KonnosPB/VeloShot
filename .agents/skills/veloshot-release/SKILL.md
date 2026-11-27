---
name: veloshot-release
description: >-
  Release, CI/CD, and Snap Store publishing workflow for VeloShot
  (GitHub Actions ci.yml, release.yml, snap.yml, dependabot.yml, branch rulesets,
  and Ubuntu Snap Store releases). Use when preparing a release, updating versions,
  troubleshooting CI checks, or publishing VeloShot to GitHub Releases or the Snap Store.
---

# VeloShot Release, CI & Snap Store Workflow

This skill documents how VeloShot's Git flow, GitHub Actions pipelines, GitHub Releases, and Ubuntu Snap Store publishing fit together.

---

## 1. Branch & GitHub Ruleset Model

- **Protected branches:** `main` (default branch) and `develop`.
- **Workflow:** Work on `develop` (or feature branches) → open Pull Request into `main` → wait for CI → merge PR → tag release `vX.Y.Z` on `main`.
- **Branch rules on `main`:**
  - Pull Request required (`0` approvals needed for solo work; admin bypass enabled).
  - Required status check: **`Build, Lint & Test`** (from `.github/workflows/ci.yml`).
  - Block force pushes & restrict deletions.
  - **Automatically delete head branches** is enabled on GitHub: after merging PRs from `develop` into `main`, GitHub deletes remote `origin/develop`. Pushing from local `develop` (`git push origin develop`) recreates `origin/develop` cleanly whenever new work starts.
- **Git identity in this repo:** Keep local git author set to:
  ```bash
  git config --local user.name "KonnosPB"
  git config --local user.email "38393288+KonnosPB@users.noreply.github.com"
  ```

---

## 2. GitHub Actions Pipelines (`.github/workflows/`)

| Workflow | File | Triggers | What it does |
| :--- | :--- | :--- | :--- |
| **CI** | `.github/workflows/ci.yml` | `push` / `pull_request` on `main` & `develop`, `workflow_dispatch` | Installs `libgtk-4-dev`, `libadwaita-1-dev`, `pkg-config`, `libdbus-1-dev`; runs `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, and `cargo build --release`. |
| **Release** | `.github/workflows/release.yml` | Push tag `v*`, `workflow_dispatch` | Tests & builds release binary on `ubuntu-24.04`, packages `veloshot-<tag>-x86_64-linux-gnu.tar.gz` (binary + `install-local.sh` + `data/` + `LICENSE` + `README.md`) + `SHA256SUMS.txt`, and publishes a GitHub Release on `v*` tags. |
| **Build and Publish Snap** | `.github/workflows/snap.yml` | Push tag `v*`, `workflow_dispatch` | Builds strict `core24` snap via `snapcore/action-build@v1` using `snap/snapcraft.yaml`, then publishes via `snapcore/action-publish@v1` to `candidate` (on `v*` tags) or the chosen `release_channel` (`edge`, `beta`, `candidate`, `stable`, `none`). |
| **Dependabot** | `.github/dependabot.yml` | Weekly schedule | Opens PRs targeting `develop` for outdated `cargo` crates and `github-actions` versions. |

### Why `ubuntu-24.04` Builds Work on Ubuntu 26.04+
Linux binaries are forward-compatible with newer `glibc` versions. Building on `ubuntu-24.04` ensures the binary works on both Ubuntu 24.04 LTS and Ubuntu 26.04 LTS+, which is why the tarball uses the distro-neutral name `veloshot-<tag>-x86_64-linux-gnu.tar.gz`.

---

## 3. Snap Store Setup (`snap/snapcraft.yaml` & `SNAPCRAFT_STORE_CREDENTIALS`)

- **Registered snap name:** `veloshot` (https://snapcraft.io/veloshot).
- **Important `snap/snapcraft.yaml` settings (`core24` + Rust Edition 2024):**
  - `grade: stable` (required for `candidate` and `stable` channels).
  - `confinement: strict` with `extensions: [gnome]` and plugs `desktop`, `desktop-legacy`, `wayland`, `x11`, `opengl`, `home`.
  - `apps.veloshot.command: bin/veloshot` (where Snapcraft's `core24` `rust` plugin installs the binary).
  - `rust-channel: "stable"` without a `rust-deps` part: Snapcraft 9 (`core24`) automatically installs the `rustup` build-snap, selects the `stable` toolchain (supporting `edition = "2024"`), and sets `PATH`. Do **not** combine `after: [rust-deps]` with `rust-channel: "stable"`, as `craft-parts` rejects that combination during environment validation.
  - `build-packages`: `pkg-config`, `libgtk-4-dev`, `libadwaita-1-dev`, `libdbus-1-dev`.
- **Store authentication:** Stored in GitHub repository secret **`SNAPCRAFT_STORE_CREDENTIALS`** (generated once via `snapcraft export-login --snaps=veloshot --acls package_access,package_push,package_update,package_release -`).
- **Snapcraft dashboard note:** The **Builds / Listing / Releases** management UI only unlocks after Revision #1 has been uploaded to the Snap Store (e.g., by running `Build and Publish Snap` manually with channel `edge`).

---

## 4. Standard Release Checklist (`vX.Y.Z`)

1. **Pre-flight checks locally on `develop`:**
   ```bash
   cargo fmt --all -- --check
   cargo clippy --all-targets -- -D warnings
   cargo test --all-targets
   cargo build --release
   ./install-local.sh --skip-build   # Optional: test updated local desktop binary
   ```
2. **Bump versions consistently:**
   - `Cargo.toml` (`version = "X.Y.Z"`)
   - `snap/snapcraft.yaml` (`version: "X.Y.Z"`)
   - `data/io.github.konnospb.VeloShot.metainfo.xml` (`<release version="X.Y.Z" date="YYYY-MM-DD">`)
   - `CHANGELOG.md` (move items from `[Unreleased]` to `[X.Y.Z] - YYYY-MM-DD`)
3. **Merge `develop` → `main` via PR:**
   - Commit & push to `origin/develop`, open PR into `main`, wait until **CI / Build, Lint & Test** is green, and merge.
4. **Tag & release from `main`:**
   ```bash
   git checkout main
   git pull origin main
   git tag -a vX.Y.Z -m "VeloShot vX.Y.Z"
   git push origin vX.Y.Z
   git checkout develop
   ```
   Pushing `vX.Y.Z` automatically triggers **both** `Release` (GitHub Release tarball + checksums) and `Build and Publish Snap` (`candidate` channel).
5. **Verify & promote Snap:**
   - Install & test `sudo snap install veloshot --candidate` (or `--edge`).
   - Promote from `candidate` to `stable` in https://snapcraft.io/veloshot/releases (or via `workflow_dispatch`).

---

## 5. Known Gotchas

- **`workflow_dispatch` visibility:** GitHub Actions only registers a manually triggerable workflow after the `.github/workflows/*.yml` file is merged into the default branch (`main`).
- **Newer `rustc` / Clippy in CI:** GitHub `ubuntu-24.04` uses latest `stable` Rust via `rustup` (newer than Ubuntu system `rustc`), so Clippy may enforce stricter lints in CI.
- **Avoid font-metric assumptions in tests:** System default `Sans` font metrics differ between Ubuntu 24.04 CI runners and local Ubuntu 26.04 machines. Sample shape geometry away from rendered text in Cairo render tests.
