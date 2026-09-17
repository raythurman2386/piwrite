# Piwrite

Markdown writing for the [pi suite](https://github.com/raythurman2386), built with
[GPUI Kit](https://github.com/longbridge/gpui-kit) — a focused writing surface for
Raspberry Pi 5-class hardware (and happy on any Linux desktop). Syntax-colored
Markdown, draft recovery, and an editor that stays out of the way.

## Features

- **Writing surface**: Markdown editor with syntax coloring, smart `enter`
  (continues lists and quotes, stays plain inside code fences), word count in
  the status bar, and `Ctrl+B` / `Ctrl+I` / `Ctrl+K` to wrap the selection in
  bold, italic, or link Markdown (a URL on the clipboard fills the link).
- **Files**: open and save Markdown through the desktop file picker, save as,
  and `Ctrl+N` for a second window. Registered as the system handler for
  `text/markdown`.
- **Find and replace**: incremental search with match counting, find-and-replace
  with replace-all, case-insensitive by default.
- **Safety net**: unsaved drafts are recovered after an abnormal exit; open
  files are watched and a prompt appears before an external change (or a
  delete) can replace local work; saves are atomic.
- **Printing**: `Ctrl+P` sends the document to CUPS (`lp`), or opens a preview
  file when no printer is configured.
- **Aesthetic**: keyboard-first, follows the desktop dark/light mode and text
  scale, and live re-tints from the Omarchy theme palette.

## Install

User-local install from a tagged release (no root, Ed25519-verified,
fail-closed):

```sh
curl -fsSL https://raw.githubusercontent.com/raythurman2386/piwrite/main/scripts/netinstall.sh | bash
```

Or build and install from source:

```sh
cargo build --release
./scripts/install.sh
```

Uninstall with `./scripts/uninstall.sh`. The netinstaller accepts a `--prefix`
directory, an optional version argument, and `--force`; the source install
honors `PREFIX=DIR`.

Tagged `v*` releases also build x86_64 + aarch64 tarballs on GitHub Actions
(glibc 2.39+ — e.g. Raspberry Pi OS / Debian 13). Unpack the one for your
architecture and run `./install.sh` inside.

Releases are authenticated with Ed25519 signatures over `checksums.txt`; the
public key is committed as `piwrite-signing-key.pub` and pinned in the
installer, which refuses anything it cannot verify.

## Keyboard

| Keys | Action |
|---|---|
| `Ctrl+S` / `Ctrl+Shift+S` | Save · save as |
| `Ctrl+O` / `Ctrl+N` | Open · new window |
| `Ctrl+F` | Find (`Enter` / `Ctrl+G` next, `Shift+Enter` previous) |
| `Ctrl+H` | Find and replace |
| `Ctrl+B` / `Ctrl+I` / `Ctrl+K` | Bold · italic · link |
| `Ctrl+P` | Print (CUPS `lp`, or a preview file) |
| `Ctrl+Z` / `Ctrl+Y` | Undo · redo (editor defaults) |
| `F11` / `Super+F` | Fullscreen |
| `Ctrl+Shift+/` | Shortcut reference · `Ctrl+Q` quit |

## State and theming

- Settings live in `~/.config/piwrite/settings.json`; recovery drafts in
  `~/.local/share/piwrite/`. Recovery slots are claimed by a lock-file scheme,
  so a crashed session's draft is picked up on the next start.
- Colors follow the desktop theme —
  `~/.local/state/omarchy/current/theme/colors.toml` when present — re-tinting
  live on theme switches; text follows the desktop text scale
  (`gsettings` `text-scaling-factor`). The default scale of `1.0` is the size
  the layout is designed around.

## Fonts

The iA Writer Mono font is bundled under the SIL Open Font License 1.1; see
`fonts/OFL.txt`. The font is copyright Information Architects Inc. and based
on IBM Plex, copyright IBM Corp.

## Development

```sh
cargo fmt --check          # formatting
cargo clippy --all-targets -- -D warnings
cargo test                 # 18 tests
cargo run --release        # write
cargo run --release -- path/to/notes.md
```

CI runs fmt, clippy, and tests on every push; tagged `v*` releases build
x86_64 + aarch64 tarballs (glibc 2.39+) with an install smoke test, and the
netinstall integrity harness can be run locally with
`bash scripts/test-netinstall.sh`.

## License

MIT — see [LICENSE](LICENSE). Bundled fonts: SIL OFL 1.1 (see above).