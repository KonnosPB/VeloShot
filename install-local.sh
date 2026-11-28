#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$(readlink -f "$0")")"

SKIP_BUILD=0
USE_SYMLINK=0
ENABLE_AUTOSTART=1

for arg in "$@"; do
    case "$arg" in
        --skip-build)
            SKIP_BUILD=1
            ;;
        --symlink)
            USE_SYMLINK=1
            ;;
        --no-autostart)
            ENABLE_AUTOSTART=0
            ;;
        -h|--help)
            cat <<'EOF'
Usage: ./install-local.sh [OPTIONS]

Builds (unless --skip-build) and installs VeloShot into ~/.local, stops any
older background daemon, configures XDG autostart (unless --no-autostart),
updates desktop/icon caches, and starts veloshot daemon.

Options:
  --skip-build     Use existing target/release/veloshot without rebuilding
  --symlink        Symlink ~/.local/bin/veloshot to target/release/veloshot
  --no-autostart   Do not install ~/.config/autostart/io.github.konnospb.VeloShot.desktop
  -h, --help       Show this help
EOF
            exit 0
            ;;
        *)
            echo "Unknown option: $arg" >&2
            exit 1
            ;;
    esac
done

if [[ "$SKIP_BUILD" -eq 0 ]]; then
    echo "Building VeloShot (release)..."
    cargo build --release
fi

if [[ ! -x "target/release/veloshot" ]]; then
    echo "Missing binary: target/release/veloshot (run without --skip-build)" >&2
    exit 1
fi

echo "Stopping running background daemon (if any)..."
~/.local/bin/veloshot quit >/dev/null 2>&1 || true
if command -v veloshot >/dev/null 2>&1; then
    veloshot quit >/dev/null 2>&1 || true
fi
sleep 0.2

mkdir -p ~/.local/bin \
         ~/.local/share/applications \
         ~/.local/share/icons/hicolor/scalable/apps \
         ~/.local/share/icons/hicolor/symbolic/apps

if [[ "$USE_SYMLINK" -eq 1 ]]; then
    ln -sf "$PWD/target/release/veloshot" ~/.local/bin/veloshot
else
    install -Dm755 target/release/veloshot ~/.local/bin/veloshot
fi

install -Dm644 data/io.github.konnospb.VeloShot.desktop \
    ~/.local/share/applications/io.github.konnospb.VeloShot.desktop
install -Dm644 data/icons/hicolor/scalable/apps/io.github.konnospb.VeloShot.svg \
    ~/.local/share/icons/hicolor/scalable/apps/io.github.konnospb.VeloShot.svg
install -Dm644 data/icons/hicolor/symbolic/apps/io.github.konnospb.VeloShot-symbolic.svg \
    ~/.local/share/icons/hicolor/symbolic/apps/io.github.konnospb.VeloShot-symbolic.svg

if [[ "$ENABLE_AUTOSTART" -eq 1 ]]; then
    mkdir -p ~/.config/autostart
    cat > ~/.config/autostart/io.github.konnospb.VeloShot.desktop <<EOF
[Desktop Entry]
Type=Application
Name=VeloShot
Comment=Fast Ubuntu Wayland screenshot tool
Exec="$HOME/.local/bin/veloshot" daemon
Icon=io.github.konnospb.VeloShot
Terminal=false
Categories=Utility;
X-GNOME-Autostart-enabled=true
EOF
else
    rm -f ~/.config/autostart/io.github.konnospb.VeloShot.desktop
fi

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t ~/.local/share/icons/hicolor >/dev/null 2>&1 || true
fi
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database ~/.local/share/applications >/dev/null 2>&1 || true
fi

echo "Starting veloshot daemon..."
nohup ~/.local/bin/veloshot daemon >/dev/null 2>&1 &

echo "VeloShot installed and daemon restarted."
