# Piwrite

A dead-simple Markdown writing app built with [GPUI Kit](https://github.com/longbridge/gpui-kit).

A focused writing surface that follows system dark/light mode, picks up Omarchy theme colors when present, recovers unsaved drafts, and stays out of the way.

## Install

User-local install (binary, icon, launcher). No root:

```sh
./scripts/install.sh
```

That puts `piwrite` on `~/.local/bin`, a desktop entry in the app launcher, and registers it for Markdown files. Then:

```sh
piwrite
piwrite path/to/notes.md
```

Uninstall with `./scripts/uninstall.sh`.

Tagged releases (`v*`) build Linux tarballs on GitHub Actions for both `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu` (glibc 2.39+ — Debian 12/13, Raspberry Pi OS, Ubuntu 24.04). Unpack the one for your architecture and run `./install.sh` inside.

## Run from source

```sh
cargo run --release
cargo run --release -- path/to/notes.md
```

## Shortcuts

- `Ctrl+S` saves. Unsaved documents use the desktop file picker.
- `Ctrl+Shift+S` saves as.
- `Ctrl+O` opens a Markdown file.
- `Ctrl+P` prints (CUPS `lp`, or a preview file if `lp` is missing).
- `Ctrl+N` opens a new Piwrite window.
- `Ctrl+Z` / `Ctrl+Shift+Z` undo and redo (editor defaults).
- `Super+F` or `F11` toggles fullscreen.
- `Ctrl+F` searches. `Enter` / `Ctrl+G` next match, `Shift+Enter` previous.
- `Ctrl+H` find and replace.
- `Ctrl+B`, `Ctrl+I`, and `Ctrl+K` insert bold, italic, and link Markdown.
- `Ctrl+Shift+/` shows the keyboard shortcut reference.

Unsaved drafts are recovered after an abnormal exit. Open files are watched, and a prompt appears before an external change can replace local work.

Text follows the desktop text size (`gsettings` `text-scaling-factor`). The default scale of `1.0` is the size the layout is designed around.

Colors come from `~/.local/state/omarchy/current/theme/colors.toml` when present.

## Fonts

The iA Writer Mono font is bundled under the SIL Open Font License 1.1; see `fonts/OFL.txt`. The font is copyright Information Architects Inc. and based on IBM Plex, copyright IBM Corp.
