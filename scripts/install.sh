#!/usr/bin/env bash
# Install Piwrite into ~/.local (binary, desktop entry, icon). No root.
set -euo pipefail

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"

cd "$ROOT"
cargo build --release --locked

install -Dm755 "$ROOT/target/release/piwrite" "$PREFIX/bin/piwrite"
install -Dm644 "$ROOT/dist/piwrite.desktop" "$PREFIX/share/applications/piwrite.desktop"
install -Dm644 "$ROOT/dist/piwrite.svg" "$PREFIX/share/icons/hicolor/scalable/apps/piwrite.svg"
install -Dm644 "$ROOT/LICENSE" "$PREFIX/share/licenses/piwrite/LICENSE"
install -Dm644 "$ROOT/fonts/OFL.txt" "$PREFIX/share/licenses/piwrite/OFL.txt"

if command -v rsvg-convert >/dev/null 2>&1; then
  tmp="$(mktemp --suffix=.png)"
  rsvg-convert -w 128 -h 128 "$ROOT/dist/piwrite.svg" -o "$tmp"
  install -Dm644 "$tmp" "$PREFIX/share/icons/hicolor/128x128/apps/piwrite.png"
  rm -f "$tmp"
elif command -v magick >/dev/null 2>&1; then
  tmp="$(mktemp --suffix=.png)"
  magick -background none "$ROOT/dist/piwrite.svg" -resize 128x128 "$tmp"
  install -Dm644 "$tmp" "$PREFIX/share/icons/hicolor/128x128/apps/piwrite.png"
  rm -f "$tmp"
fi

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$PREFIX/share/applications" >/dev/null 2>&1 || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f -t "$PREFIX/share/icons/hicolor" >/dev/null 2>&1 || true
fi
if command -v xdg-mime >/dev/null 2>&1; then
  xdg-mime default piwrite.desktop text/markdown >/dev/null 2>&1 || true
  xdg-mime default piwrite.desktop text/x-markdown >/dev/null 2>&1 || true
fi

echo "Installed piwrite to $PREFIX/bin/piwrite"
echo "Launcher: $PREFIX/share/applications/piwrite.desktop"
if [[ ":$PATH:" != *":$PREFIX/bin:"* ]]; then
  echo "Add $PREFIX/bin to PATH if the launcher cannot find piwrite."
fi
