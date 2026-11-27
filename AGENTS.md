# AGENTS.md — VeloShot

## Project Overview
**VeloShot** is a fast, fluid screenshot tool written in **Rust**, tailored for **Ubuntu (GNOME / Wayland)**, featuring an interactive **carousel history** for browsing and acting on recent screenshots.

## Verified Environment Baseline
- **OS:** Ubuntu 26.04.1 LTS (`XDG_SESSION_TYPE=wayland`, `XDG_CURRENT_DESKTOP=ubuntu:GNOME`)
- **Toolchain:** `rustc 1.94.1`, `cargo 1.94.1`
- **License:** MIT
- **System Libraries Note:** `gtk4` and `libadwaita-1` development headers (`pkg-config`) are **not** pre-installed. If the chosen UI stack requires system C libraries, document the `apt` prerequisites in `README.md` before adding them, or prefer a pure-Rust Wayland/GPU stack.

## Engineering & UX Priorities
1. **Extreme Startup Speed:** Launching capture or opening the carousel must feel instantaneous (consider background daemon + IPC / D-Bus activation, or minimal cold-start binary).
2. **Fluid 60+ FPS Carousel:** Smooth horizontal/perspective carousel with keyboard navigation (`Left`/`Right`, `Enter` to copy, `Esc` to close, `Del` to remove) and mouse/trackpad scroll.
3. **Ubuntu GNOME Wayland Compatibility:** Handle Wayland security boundaries cleanly (e.g., `xdg-desktop-portal` via `ashpd`, PipeWire, fractional scaling, multi-monitor coordinates, and Wayland clipboard persistence).
4. **Clean Rust Architecture:** Modular workspace or crate modules separating `capture`, `history`, `clipboard`, `ui`, and `daemon`/`cli`.

## Planning Handoff
See [docs/SPECIFICATION.md](docs/SPECIFICATION.md) for detailed functional requirements, non-functional targets, and open architecture decisions to resolve during planning.

## Standing Repository Rules (Always Follow)
1. **Keep Documentation Up to Date:** Whenever adding or changing user-facing features, shortcuts, CLI options, or build prerequisites, automatically update `README.md` (both German and English sections) and `CHANGELOG.md` without waiting to be asked.
2. **Secrets, Privacy & Git Identity:** Never commit secrets, tokens, private emails, or machine-specific paths (`/home/<user>/...`). Keep the repository git identity set to `KonnosPB <38393288+KonnosPB@users.noreply.github.com>`.
3. **No Competitor References:** Never mention competing screenshot products in code, UI strings, `README.md`, or store metadata.
4. **17-Language Localization:** Whenever adding or changing user-facing UI strings, update all 17 supported European languages in `src/i18n/` and ensure `i18n::tests::all_17_languages_complete_valid_markup_and_placeholders` passes.
5. **Quality Gate & Branch Flow:** Before every commit or PR, run `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --all-targets`. Work on `develop` (or feature branches) and merge into `main` via Pull Request.
