# YayGUI

**An unofficial GUI for [yay](https://github.com/Jguer/yay), the AUR helper for Arch Linux.**

> A friendly face for yay — every command visible, every PKGBUILD reviewable, nothing built as root.

YayGUI is a planned modern desktop app for search, install, update, remove and review of packages from the Arch repositories and the AUR. It is built on Tauri 2 (React and TypeScript UI, Rust backend) and is not affiliated with the yay project. "YayGUI" is a working name and will change before the first public release.

## Status

**Pre-alpha.** The repository currently contains early spikes and crates only. There is no usable release yet.

No AUR package exists yet. The planned `yaygui`, `yaygui-bin` and `yaygui-git` packages are not published, so `yay -S yaygui-bin` does not work.

## Safety principles

These are hard rules, enforced in code and covered by tests:

1. **Never runs as root.** The app refuses to start as UID 0.
2. **No partial upgrades.** Repository installs always go through `-S --needed` after a full `-Syu`, or the app asks you to upgrade first.
3. **No silent AUR builds.** Every new AUR package and every AUR update shows its PKGBUILD or a diff since the last approved version before anything is built.
4. **Credentials are never stored.** Sudo passwords go straight to the terminal and are wiped from memory. AUR account credentials, if ever used, go to the system keyring.
5. **No shell string building.** Commands are spawned with argument arrays, and package names are validated before use.

## Planned features

- **MVP:** unified search across repos and the AUR with source badges; package detail pages; an install and remove queue with one review step; PKGBUILD and diff viewer; updates dashboard; full system upgrade only; orphan cleanup; a built-in terminal pane showing the exact yay output.
- **v1.0:** feature parity with the daily-use yay commands (`-S`, `-Syu`, `-R`, `-Ss`, `-Si`, `-Qu`, `-Yc`, `-Sc`, `-G`, `-Ps`), Arch News check before upgrades, risk signals for AUR packages, a yay configuration editor, cache manager, light and dark themes, and i18n.

The full plan, including the architecture and the review-pinning design, is in [PLAN.md](PLAN.md). Architecture decisions are recorded in [docs/adr](docs/adr/README.md).

## How it works

Reads never go through yay. Package data comes from libalpm (read-only) and the AUR RPC. Every change to the system goes through yay in a real pseudo-terminal, so you always see the command and its output.

## Repository layout

```
crates/core          domain types, transaction planner, safety rules (no Tauri dependency)
crates/alpm-reader   read-only libalpm queries
crates/aur-client    AUR RPC v5 client, batching, caching
crates/yay-runner    PTY spawning, output handling, prompt detection, transaction runner
crates/review        AUR git fetch, PKGBUILD diffs, review history
apps/desktop         Tauri 2 app: React + TypeScript UI and Rust commands
docs/adr             architecture decision records
```

## Development

Requirements: a stable Rust toolchain (1.88 or newer), Node.js 22 and pnpm.

```sh
# All CI checks locally (Rust, cargo deny, UI)
scripts/check.sh

# Rust crates
cargo test --workspace

# Desktop UI only (browser, uses mock data)
cd apps/desktop && pnpm install && pnpm dev

# Full desktop app (Tauri window + Rust backend)
cd apps/desktop && pnpm install && pnpm tauri dev
```

Building the Tauri app needs the WebKitGTK 4.1 development packages:

```sh
# Ubuntu / Debian
sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev \
  libayatana-appindicator3-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev \
  build-essential pkg-config

# Arch
sudo pacman -S --needed webkit2gtk-4.1 gtk3 librsvg libayatana-appindicator libsoup3 base-devel
```

Debug builds run the program named in `YAYGUI_YAY` instead of `yay`, so the run flow can be tried against a fake script (for example `crates/yay-runner/tests/fixtures/fake-tx.sh`). Release builds ignore it.

`cargo build --workspace` also compiles the Tauri crate, which expects `apps/desktop/dist` to exist (run `pnpm build` or `mkdir -p apps/desktop/dist`).

Anything that touches pacman or yay must be tested inside a disposable Arch container, never on your everyday system. See [CONTRIBUTING.md](CONTRIBUTING.md).

## Contributing and security

- [CONTRIBUTING.md](CONTRIBUTING.md): setup, coding style, commit format, review rules
- [SECURITY.md](SECURITY.md): how to report vulnerabilities privately
- [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md): community standards

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
