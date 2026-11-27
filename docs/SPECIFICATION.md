# VeloShot — Product & Architecture Specification

## 1. Vision
Build a modern, ultra-fast screenshot utility in **Rust** optimized for **Ubuntu Wayland (GNOME)** that combines instant capture with a fluid, visual **carousel history**.

## 2. Core Requirements

### 2.1 Capture & Ubuntu Wayland Integration
- **Capture Modes:**
  - Interactive region selection (drag-to-select, crosshair, dimensions indicator)
  - Active window / specific window capture
  - Full-screen / specific monitor capture
  - Delayed capture (optional timer)
- **Wayland Compatibility (Ubuntu GNOME):**
  - Must work reliably under `ubuntu:GNOME` on Wayland without XWayland hacks.
  - Handle multi-monitor setups and fractional scaling (`1.25x`, `1.5x`, `2x`) accurately.
  - Reliable Wayland clipboard copy (image stays available even after the UI closes, or via background daemon / `wl-data-control`).

### 2.2 Interactive Carousel History
- Display recent screenshots in a **smooth, fluid carousel** (cover-flow / horizontal card strip with center focus, smooth easing/spring physics).
- **Quick Actions on Selected Screenshot:**
  - Copy to clipboard (`Ctrl+C` / `Enter`)
  - Save / Export to file (`Ctrl+S`)
  - Quick annotate / crop / blur (phase 2)
  - Delete from history (`Delete`)
  - Pin / favorite or search/filter by timestamp (phase 2)
- **Storage:**
  - Persist screenshots and lightweight metadata (timestamp, dimensions, mode) in XDG-compliant directories (`$XDG_DATA_HOME/veloshot` / `$XDG_CACHE_HOME/veloshot`).
  - Asynchronous thumbnail decoding/caching so the carousel opens with zero stutter even with dozens/hundreds of screenshots.

### 2.3 Performance & Responsiveness
- **Startup Latency:** Target `< 100 ms` from shortcut trigger to ready state (via resident daemon + Unix socket/D-Bus IPC, or fast cold start).
- **Frame Rate:** Smooth `60+ FPS` animations in both region selection overlay and carousel view.
- **Memory Footprint:** Keep idle daemon memory minimal; evict full-resolution textures when carousel is closed.

## 3. Key Architecture Decisions (For Planning Phase)

| Decision Area | Options to Evaluate | Key Trade-offs |
| :--- | :--- | :--- |
| **Wayland Capture Backend** | `ashpd` (`xdg-desktop-portal` Screenshot / ScreenCast + PipeWire) vs. GNOME Shell DBus | Portal is standard & sandbox-friendly; GNOME ScreenCast/PipeWire allows custom instant region overlay without repeated portal permission prompts. |
| **GUI / Rendering Stack** | `iced` / `egui` (`eframe` or `wgpu`) / `slint` vs. `gtk4-rs` + `libadwaita` | Pure-Rust GPU (`iced`/`wgpu`/`slint`) gives custom 60fps carousel animations without C-header deps; `gtk4-rs` integrates natively with GNOME Adwaita. |
| **Activation Model** | Resident lightweight daemon + CLI/IPC client vs. pure single-shot binary | Daemon enables `<50ms` activation, persistent Wayland clipboard ownership, and pre-warmed history index. |
| **Image & Storage Format** | PNG/WebP + SQLite or JSON/RON index + async thumbnail cache | PNG for lossless clipboard/export; WebP/QOI or fast-decode thumbnails for instant carousel scrolling. |

## 4. Suggested Milestone Roadmap
1. **M1 — Architecture & Prototype:** Decide capture backend (`ashpd` / PipeWire) + UI toolkit; verify Wayland capture & clipboard on Ubuntu 26.04 GNOME.
2. **M2 — Core Capture & Storage:** Implement region/fullscreen capture, XDG history persistence, and fast thumbnail cache.
3. **M3 — Carousel UI:** Build the 60fps interactive carousel with keyboard-first navigation and instant clipboard copy.
4. **M4 — Daemon & Polish:** Add single-instance daemon for instant startup, desktop entry, GNOME shortcut integration, and packaging (`.deb` / Cargo).
