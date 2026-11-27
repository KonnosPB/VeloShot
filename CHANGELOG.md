# Changelog

All notable changes to **VeloShot** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

- Added GitHub Actions CI (`rustfmt`, `clippy`, `test`, `build`), automated `v*` release workflow, Dependabot, issue/PR templates, and community documentation (`SECURITY.md`, `CONTRIBUTING.md`, `CHANGELOG.md`).

## [0.1.0] - 2026-11-27

### Added
- **Wayland Region Capture & Floating Mini-Bar:** GPU-rendered fullscreen overlay via `xdg-desktop-portal` (`ashpd`) with 6× live zoom loupe, monitor picker with hover highlight, window picker, 3s/5s capture timers, and movable mini-bar.
- **Interactive 60+ FPS Carousel History:** Large live preview with 60% translucent maximized backdrop, configurable placement (**Top**, **Bottom**, **Center**), keyboard/scroll navigation, and spring-animated **Top ↔ Bottom** FLIP toggle (`F` / `Tab`).
- **Built-in Annotation Editor:** Select/Move (`V`), Rectangle (`1`), Arrow (`2`), Ellipse (`3`), Highlighter (`4`), Freehand (`5`), Numbered Steps (`6`), Text (`7`), Speech Bubble with centered text & auto canvas expansion (`8`), Pixelate (`9`), Spotlight (`0`), Crop (`C`), background fills, 90° rotation & horizontal flip, 10 Beautify gradient frame presets, scalable icon sizes, Undo/Redo, zoom, and Wayland auto-copy.
- **Offline OCR (Optional):** Tesseract text recognition tailored dynamically to all 17 supported UI languages (`tesseract-ocr-<code> + eng`) with one-click installer and setup guide in Preferences.
- **17 European Languages:** Runtime localization with `Auto (System)` detection and instant in-place switching in Preferences.
- **Desktop Integration & Packaging:** Automatic shortcut setup for **GNOME**, **KDE Plasma**, **XFCE**, and **Sway**, PNG/JPEG export with date-first filename patterns, `install-local.sh`, Snap metadata, and bilingual documentation.
