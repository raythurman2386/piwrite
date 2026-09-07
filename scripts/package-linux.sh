#!/usr/bin/env bash
# Build a Linux x86_64 release tarball: binary, desktop entry, icon, install script.
set -euo pipefail

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
VERSION="${VERSION:-$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)}"
ARCH="${ARCH:-x86_64}"
TARGET="${TARGET:-${ARCH}-unknown-linux-gnu}"
BIN="${PREBUILT_BIN:-$ROOT/target/release/piwrite}"
OUT_DIR="${OUT_DIR:-$ROOT/target/package}"
NAME="piwrite-${VERSION}-${TARGET}"
STAGE="$OUT_DIR/$NAME"

if [[ ! -x "$BIN" ]]; then
  echo "package-linux: missing binary at $BIN (build --release first)" >&2
  exit 1
fi

rm -rf "$STAGE"
mkdir -p "$STAGE"

install -m 755 "$BIN" "$STAGE/piwrite"
install -m 644 "$ROOT/dist/piwrite.desktop" "$STAGE/piwrite.desktop"
install -m 644 "$ROOT/dist/piwrite.svg" "$STAGE/piwrite.svg"
install -m 644 "$ROOT/LICENSE" "$STAGE/LICENSE"
install -m 644 "$ROOT/fonts/OFL.txt" "$STAGE/OFL.txt"

cat >"$STAGE/install.sh" <<'INSTALL'
#!/usr/bin/env bash
# Install a prebuilt Piwrite release into ~/.local. No root, no compiler.
set -euo pipefail
HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"

install -Dm755 "$HERE/piwrite" "$PREFIX/bin/piwrite"
install -Dm644 "$HERE/piwrite.desktop" "$PREFIX/share/applications/piwrite.desktop"
install -Dm644 "$HERE/piwrite.svg" "$PREFIX/share/icons/hicolor/scalable/apps/piwrite.svg"
install -Dm644 "$HERE/LICENSE" "$PREFIX/share/licenses/piwrite/LICENSE"
install -Dm644 "$HERE/OFL.txt" "$PREFIX/share/licenses/piwrite/OFL.txt"

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
INSTALL
chmod 755 "$STAGE/install.sh"

mkdir -p "$OUT_DIR"
TARBALL="$OUT_DIR/${NAME}.tar.gz"
tar -czf "$TARBALL" -C "$OUT_DIR" "$NAME"
echo "packaged: $TARBALL"
