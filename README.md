# Piwrite

A dead-simple Markdown writing app built with [GPUI Kit](https://github.com/longbridge/gpui-kit).

A focused writing surface that follows system dark/light mode, picks up Omarchy theme colors when present, recovers unsaved drafts, and stays out of the way.

## Install

### Netinstaller (recommended)

One line — downloads the latest release for your architecture, verifies its Ed25519 signature against the pinned public key and its SHA-256 against the signed checksums, and refuses to install anything that fails either check:

```sh
curl -fsSL https://raw.githubusercontent.com/raythurman2386/piwrite/main/scripts/netinstall.sh | bash
```

Or clone and run (same verification, easier to read before running):

```sh
git clone https://github.com/raythurman2386/piwrite
./piwrite/scripts/netinstall.sh
```

Both put `piwrite` on `~/.local/bin`, install the desktop entry, icon, and Markdown file associations, and accept an optional version argument (`./netinstall.sh 0.1.3`) plus `--force` to overwrite an existing install.

### From source

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

## Release signing

Every release's `checksums.txt` is signed with an Ed25519 key. The signature (`checksums.txt.sig`) is attached to the release; the public key is committed at `piwrite-signing-key.pub` and pinned in `scripts/netinstall.sh`, which refuses to install a release it cannot authenticate. The secret key is kept offline — it is never committed, never used in CI, and never uploaded.

Maintainer flow (local machine only, `openssl` + `gh` required):

```sh
# once: generate the keypair into ~/.piwrite/signing
./scripts/gen-signing-key.sh

# after a release publishes: sign its checksums.txt offline, then attach
./scripts/sign-releases.sh v0.1.3
./scripts/upload-release-sigs.sh v0.1.3
```

Signing and uploading are separate scripts so the secret key never touches a network call. Verify a release's signature by hand:

```sh
gh release download v0.1.3 -p 'checksums.txt*'
openssl pkeyutl -verify -rawin -in checksums.txt -sigfile checksums.txt.sig -pubin -inkey piwrite-signing-key.pub
```

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
