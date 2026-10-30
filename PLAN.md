# YayGUI — Project Plan

Oct 27, 2026 · @eHouse

## Vision and goals

Build a modern, safe, native-feeling desktop app that does everything `yay` does from the terminal — search, install, update, remove, review PKGBUILDs, manage the cache and config — without ever hiding what is actually running. Working name: **YayGUI** (rename before the first public release; avoid using "yay" as the brand so it is not mistaken for an official upstream project).

**Who it's for**

- Arch, EndeavourOS, CachyOS and Garuda users who already use yay but want a visual way to browse, compare and batch-manage packages.
- Newer Arch users who need the AUR but are not yet comfortable reading PKGBUILDs in a terminal — the app should teach them, not shield them.
- Power users who want a dashboard: pending updates, orphans, cache size, foreign packages, news, in one place.

**Design principles**

1. **Transparent, not magic.** Every action shows the exact `yay`/`pacman` command it will run, and a live terminal pane shows the output.
2. **Safe by default.** No partial upgrades, PKGBUILD review is prominent, nothing builds as root.
3. **Thin wrapper, not a fork.** Use yay for AUR building and pacman/libalpm for reads; never re-implement dependency solving.
4. **Fast.** Search results under 300 ms for repo packages; the app opens in under 1 second.
5. **Native look.** Follows the system theme (light/dark, accent colour) on KDE and GNOME.

**Success criteria for v1.0**

- Feature parity with the yay commands people use daily (`-S`, `-Syu`, `-R`, `-Ss`, `-Si`, `-Qu`, `-Yc`, `-Sc`, `-G`, `-Ps`).
- Zero known paths that build as root or perform a partial upgrade.
- Packaged in the AUR, 50+ AUR votes and 5+ outside contributors within six months of 1.0.

## Landscape and the gap

There is room for this project: the established Arch GUIs are either Manjaro-centric, built on aging Qt5/GTK3 stacks, or treat the AUR as an add-on, and none is designed around yay itself. The Arch Wiki lists Pamac, Octopi, Bauh and a few smaller wrappers as the main graphical front-ends ([Arch Wiki: AUR helpers](https://wiki.archlinux.org/title/AUR_helpers)).

| Project | Stack | AUR approach | Weak spot this project can beat |
| --- | --- | --- | --- |
| [Pamac](https://wiki.archlinux.org/title/AUR_helpers) | GTK 4, Vala, libalpm | Own AUR builder | Built for Manjaro; community reports recurring breakage on plain Arch |
| [Octopi](https://alternativeto.net/software/octopi) | Qt 5, C++ | Calls your installed AUR helper | Dated UI; AUR is a secondary mode |
| [Bauh](https://www.saashub.com/compare-bauh-vs-pamac) | Qt 5, Python | Own AUR logic, plus Flatpak/Snap/AppImage | Generalist; forum users warn about embedded partial-upgrade behaviour |
| [parui](https://aur.archlinux.org/packages/?K=yay) | Rust TUI | Front-end for paru or yay | Terminal only |
| [yay-gui-manager-git](https://aur.archlinux.org/packages/?K=yay) | Unknown | yay wrapper | Uploaded Dec 2025, no votes — an early experiment, not a competitor |

**The gap:** a modern (GTK 4/libadwaita or Qt 6 class) app that is yay-native, plays well with yay v13's new Lua hooks, and makes PKGBUILD review the centre of the AUR experience rather than a dialog people click through. Forum threads show people want "Pamac on Arch without the breakage" ([EndeavourOS forum](https://forum.endeavouros.com/t/which-gui-package-manager-should-i-install/33118), [CachyOS forum](https://discuss.cachyos.org/t/what-gui-package-manager-do-you-use/10333)).

**Positioning line for the README:** "A friendly face for yay — every command visible, every PKGBUILD reviewable, nothing built as root."

## Feature set

Ship in three tiers: an MVP that replaces daily terminal use (search, install, update, remove), a v1.0 that covers everything yay does, and a post-1.0 tier for the features no other GUI has. The command column is what the app runs or reads under the hood.

| Area | Feature | Tier | Under the hood |
| --- | --- | --- | --- |
| Discover | Unified search across repos + AUR, with source badges, votes, popularity, last-updated age | MVP | libalpm sync DBs + AUR RPC v5 `search` |
| Discover | Package detail page: description, deps tree, files, optdeps, changelog, upstream link | MVP | libalpm + AUR RPC `info` |
| Discover | Browse by repo, group, category; "recently updated" and "new in AUR" feeds | v1.0 | AUR metadata dump, cached locally |
| Discover | AUR comments viewer (read-only), pinned comments first | v1.0 | AUR package page |
| Install | Install from repos and AUR, batch queue ("cart") with one review step for all | MVP | `yay -S --needed` |
| Install | PKGBUILD + .install + diff viewer before every AUR build | MVP | `yay -G` / `-Gp`, git diff in yay cache |
| Install | Optional-dependency picker after install | v1.0 | libalpm optdepends |
| Install | Install local PKGBUILD folder | Later | `yay -Bi <dir>` |
| Update | Updates dashboard: repo vs AUR vs -git (devel) packages, sizes, changelogs | MVP | `yay -Qu`, `yay -Syu --devel` |
| Update | Full system upgrade only — no per-package repo upgrades (prevents partial upgrades) | MVP | `yay -Syu` |
| Update | Arch News check before upgrade; block if unread manual-intervention item | v1.0 | archlinux.org news RSS |
| Update | Tray/notifier with update count, configurable interval | v1.0 | `checkupdates` + AUR info, no root |
| Update | Ignore/hold packages (IgnorePkg) | v1.0 | pacman.conf / yay config |
| Remove | Remove with or without deps and config | MVP | `yay -R`, `-Rs`, `-Rns` |
| Remove | Orphan finder and cleanup | MVP | `yay -Yc` |
| Maintain | Cache manager: size, clean pacman + yay caches | v1.0 | `yay -Sc`, `paccache` |
| Maintain | Foreign/AUR package audit list with maintainer and age | v1.0 | `pacman -Qm` + AUR info |
| Maintain | System stats page | v1.0 | `yay -Ps` |
| Maintain | Transaction history and rollback hints (downgrade from cache) | Later | pacman.log parser |
| Maintain | Mirror ranking helper | Later | `reflector` if installed |
| Configure | Visual editor for yay options and Lua hooks, raw-file mode for experts | v1.0 | `~/.config/yay/init.lua` |
| Configure | pacman.conf viewer (read-only by default; editing via polkit) | Later | `/etc/pacman.conf` |
| Safety | Risk signals per AUR package: maintainer changed recently, package age, orphan, out-of-date, low votes | v1.0 | AUR RPC fields + local history |
| Safety | Optional automated PKGBUILD scan hook (pluggable scanners) | Later | yay Lua `AURPostDownload`-style hook |
| UX | Built-in terminal pane with full yay output; copy command button | MVP | PTY |
| UX | Light/dark, keyboard shortcuts, i18n (Bengali among first languages) | v1.0 | — |
| AUR account | Vote/unvote | Later | `yay -Wv` / `-Wu` (needs AUR credentials) |

Partial upgrades are deliberately never offered: installing one repo package after a sync without upgrading the rest is the most common way GUIs break Arch systems.

## Architecture

The app splits reads from writes: package data comes straight from libalpm and the AUR API, while every change to the system goes through yay in a pseudo-terminal. That keeps the UI fast and avoids depending on yay's human-readable output for anything but progress.

&#91;embedded content: architecture · UI, Rust backend, system tools\]

The UI never touches the system directly; the backend reads data itself and hands every change to yay, which escalates only for pacman's install step.

**Key design decisions**

- **Reads never go through yay.** Search, details, installed lists and dependency trees come from libalpm (read-only, no root) and AUR RPC v5. Update checks use `checkupdates` for repos and AUR `info` + libalpm `vercmp` for AUR packages.
- **Writes always go through yay, one at a time.** `core` turns the queue into a single yay invocation with an argument array, e.g. `yay -S --needed --answerdiff None --answeredit None pkg1 pkg2` after the GUI review. pacman's lock file allows only one transaction anyway; the backend watches `/var/lib/pacman/db.lck` and explains when another tool holds it.
- **Real PTY, not pipes.** yay, makepkg and pacman behave differently without a terminal. The raw byte stream goes to xterm.js unchanged; a light parser recognises prompts (`[sudo] password`, pacman `[Y/n]` questions such as replacements and conflicts) and surfaces them as native dialogs rather than answering blindly with `--noconfirm`.
- **Transaction state machine:** Planned → Reviewing → Approved → Running → Succeeded / Failed / Cancelled. Each state is persisted, so a crash mid-transaction shows what happened on next launch.
- **Local data:** one SQLite file in `~/.local/share/<app>/` for review history (package, maintainer, approved commit, date) and an AUR metadata cache; settings in `~/.config/<app>/`. The user's yay config is never rewritten — the app layers its own `init.lua` overlay on top via a private `XDG_CONFIG_HOME` per run.
- **Graceful degradation:** with yay 12.x the app still works, but review pinning is disabled and the UI says so.

## Tech stack

**Recommendation: Tauri 2 + React + TypeScript for the UI, Rust for the backend.** It lets you build the whole front end with skills you already have, keeps the binary small (it uses the system WebKitGTK instead of bundling Chromium), and gives access to mature Rust libraries for libalpm and terminals.

| Option | UI | Backend | Pros | Cons |
| --- | --- | --- | --- | --- |
| **Tauri 2** (recommended) | React + TS + Vite | Rust | Your existing skills on the UI; small footprint; strict permission (capabilities) model; [alpm.rs](https://docs.rs/crate/alpm/latest) bindings maintained alongside paru | Rust learning curve; WebKitGTK quirks on some GPUs; not pixel-native |
| Wails | React + TS | Go | Go is easier than Rust; can reuse yay's own Go libraries (go-alpm, Jguer/aur) | Smaller ecosystem than Tauri; fewer plugins |
| GTK 4 + libadwaita | Native widgets | Rust (gtk-rs / Relm4) or Python | Truly native on GNOME; best accessibility | New UI toolkit to learn; looks foreign on KDE |
| Qt 6 / QML | Native widgets | C++ or Rust (cxx-qt) | Native on KDE; Octopi-proven | C++ or complex bindings; Qt licensing questions for some contributors |
| Electron | React + TS | Node | Fastest to start for you | 150 MB+ install, high RAM — Arch users will reject it |

**Concrete stack**

- **UI:** React 19, TypeScript, Vite, TanStack Query (cache + refetch), TanStack Table/Virtual (long package lists), Zustand (UI state), Tailwind + Radix primitives styled to resemble libadwaita/Breeze, xterm.js for the terminal pane, Monaco or CodeMirror 6 for the PKGBUILD/diff viewer (CodeMirror is lighter).
- **Backend (Rust):** `tauri` 2, `alpm` crate for read-only local/sync DB queries, `reqwest` + `serde` for AUR RPC v5, `portable-pty` to run yay in a real pseudo-terminal, `tokio` for async, `notify` to watch pacman's lock file and log, `zbus` for D-Bus (polkit, desktop notifications).
- **Separate notifier:** a tiny Rust tray binary (`ksni` / StatusNotifierItem) so update checks do not need the full app open.
- **Tooling:** pnpm, cargo, Biome or ESLint + Prettier, `cargo clippy`, `cargo deny`, Vitest + Playwright for UI, `cargo test` for the backend.

If Rust feels too steep, Wails with Go is the fallback; the architecture below works the same either way.

## Security model

Security is the feature that will make or break adoption: in June 2026 more than 1,500 AUR packages were compromised in three waves, mostly through attackers adopting orphaned packages and pushing malicious updates ([It's FOSS](https://itsfoss.com/news/arch-linux-aur-malware-flood.md), [Phoronix](https://phoronix.com/news/arch-linux-aur-more-than-1500)). A further wave from 29 July hit 200+ packages and led Arch to freeze package adoption ([ThreatCluster](https://threatcluster.io/cluster/arch-linux-freezes-aur-adoption-tor-1f0be90b)). A GUI that makes installing AUR packages *easier* must make reviewing them easier too.

**Hard rules (enforced in code, covered by tests)**

1. **The app never runs as root.** It refuses to start as UID 0. makepkg refuses root anyway, and yay itself warns against it.
2. **No partial upgrades.** Repo installs always go through `-S --needed` after a full `-Syu`, or the UI asks to upgrade first. Update checks use `checkupdates` (temporary DB copy), never `pacman -Sy`.
3. **No silent AUR builds.** Every new AUR package and every AUR update shows the PKGBUILD (first install) or a diff since the last installed version (updates) before building. "Skip review" exists only as an explicit per-session opt-in with a warning.
4. **Credentials are never stored.** Sudo passwords pass straight into the PTY and are wiped from memory; AUR account credentials (for voting) go to the system keyring via Secret Service.
5. **No shell string building.** Commands are spawned with argument arrays; package names are validated against the AUR/pacman name pattern before use.

**Privilege escalation**

- Default: yay runs in a PTY; when the backend detects a sudo password prompt, the UI shows a native password field that writes into the PTY. Simple, honest, works with any sudo/doas setup.
- Option: a polkit mode passing yay's `--sudo` / `--sudoflags` settings so pacman steps show the system authentication dialog instead. Validate carefully with yay's sudo loop before making it default.
- Never ship a setuid helper or a root daemon in v1. (Pamac's root daemon is a large attack surface; revisit only if there is a strong need.)

**Review pinning — what you reviewed is what gets built**

yay v13 added Lua hooks: `AURPreInstall` fires after PKGBUILD repos are fetched and before diff/edit/build, `AURPostDownload` after `makepkg --verifysource`, `UpgradeSelect` during `-Syu` before the exclusion menu, plus search-filter and post-install hooks ([Linuxiac](https://linuxiac.com/yay-13-0-adds-new-review-and-automation-features/), [It's FOSS](https://itsfoss.com/news/yay-v13-release.md)). The app can use them to close the gap between review and build:

1. The app fetches the PKGBUILD repo into yay's cache, shows the diff, and records the git commit hash the user approved.
2. It runs yay with a private `XDG_CONFIG_HOME` overlay whose `init.lua` imports the user's own config and adds an `AURPreInstall` hook that aborts if the fetched commit differs from the approved one. (The meta-package-manager project already uses this overlay technique for its cooldown feature — [mpm docs](https://kdeldycke.github.io/meta-package-manager/managers/yay.html).)
3. Because the review already happened in the GUI, yay can then run with `--answerdiff None --answeredit None` without losing safety.

Open question: confirm from yay's Lua documentation which event fields (commit hash, PKGBUILD path) each hook receives, and whether a hook can abort with a clear message. File an upstream issue early if a field is missing.

**Risk signals shown on every AUR package**

- Maintainer changed since you last installed it (stored in a local SQLite history) — the exact pattern used in the 2026 attacks.
- PKGBUILD last-modified age (yay v13 now surfaces this too), first-submitted date, votes, popularity, orphaned / out-of-date flags.
- New network fetches or `curl | sh`, `npm`/`bun` installs, base64 blobs or obfuscated strings in the diff, highlighted by simple local heuristics. These are hints, never a "safe" badge.
- Pluggable external scanners later (e.g. an `AURPostDownload` hook running a user-chosen tool), always opt-in.

**Supply chain of the app itself**

Signed git tags, reproducible release builds from CI, `cargo deny` + `pnpm audit` in CI, Dependabot/Renovate, a `SECURITY.md` with a private disclosure address, and a minimal Tauri capability set (no arbitrary shell, no remote content in the webview).

## UI and UX

A sidebar app with seven screens, a global search bar, a transaction queue that collects actions, and a terminal drawer that can slide up from the bottom at any time.

**Screens**

1. **Home / Dashboard** — update count (repo / AUR / devel), Arch News headlines, cache size, orphan count, last upgrade date, one big "Upgrade system" button.
2. **Search / Browse** — instant results with source badge (core, extra, multilib, AUR), installed state, votes, age; filters for source, installed, out-of-date, orphaned.
3. **Package detail** — overview, dependencies (tree and reverse deps), files, PKGBUILD tab (AUR), comments tab (AUR), risk signals panel, install/remove buttons.
4. **Updates** — grouped list with version from→to, download size, per-AUR-package review status (not reviewed / reviewed / changed since review).
5. **Installed** — all, explicitly installed, AUR/foreign, orphans; bulk select for removal.
6. **Maintenance** — cache cleaner, orphan cleanup, `yay -Ps` stats, pacman.log history.
7. **Settings** — app preferences, yay options editor, Lua hook manager, notifier settings, about.

**The core flow: queue → review → run**

1. User adds packages to the queue from search or detail pages (or clicks "Upgrade system").
2. Pressing **Apply** opens a summary: what will be installed, upgraded, removed, total download size, and the exact command.
3. Every AUR package in the queue must be reviewed: PKGBUILD or diff side by side, risk signals above it, "Approve" / "Remove from queue".
4. The transaction runs in the terminal drawer with a progress header; sudo prompts appear as a native password field.
5. A result screen shows success or the failing step, the log, and next actions ("Install optional deps", "Retry", "Copy log for a bug report").

**UX details that matter**

- Keyboard first: `/` focuses search, `Ctrl+Enter` applies the queue, Ctrl+J toggles the terminal drawer.
- Never block the UI: long operations run in the background with a progress pill in the header; only one pacman transaction at a time (pacman's lock enforces this anyway).
- Detect and explain the pacman lock (`/var/lib/pacman/db.lck`) instead of failing cryptically.
- Plain-language explanations next to scary terms ("orphan", "makedepends", "-git package") with a "learn more" link to the Arch Wiki.
- Empty, loading and error states designed for every screen, not just the happy path.

## Repo structure

One monorepo: a Cargo workspace for the Rust crates and a pnpm workspace for the UI. Keeping the package-management logic in its own crate (`core`) with no Tauri dependency means it can be tested headless and reused by the notifier or a future CLI.

```
yaygui/
├── apps/
│   ├── desktop/              # Tauri app
│   │   ├── src/              # React + TS UI
│   │   │   ├── features/     # search, package, updates, installed, maintenance, settings, queue
│   │   │   ├── components/   # shared UI kit (adwaita/breeze-like)
│   │   │   ├── terminal/     # xterm.js drawer
│   │   │   ├── i18n/         # en, bn, ... (JSON catalogs)
│   │   │   └── api/          # typed wrappers around Tauri commands/events
│   │   └── src-tauri/        # Rust entry, commands, capabilities, tauri.conf.json
│   └── notifier/             # tray binary (Rust)
├── crates/
│   ├── core/                 # domain types, transaction planner, safety rules
│   ├── alpm-reader/          # read-only libalpm queries (alpm crate)
│   ├── aur-client/           # AUR RPC v5 + metadata dump + rate limiting + cache
│   ├── yay-runner/           # PTY spawn, output parsing, prompt detection, overlay init.lua
│   ├── review/               # git diff, risk heuristics, review history (SQLite)
│   └── news/                 # Arch News RSS
├── packaging/
│   ├── aur/                  # PKGBUILD (release) and -git variant
│   └── icons/, desktop/, metainfo/
├── docs/                     # user guide + architecture decision records (ADRs)
├── tests/fixtures/           # recorded yay output, sample PKGBUILDs, malicious samples (defanged)
├── .github/                  # CI workflows, issue/PR templates, CODEOWNERS
├── CONTRIBUTING.md  CODE_OF_CONDUCT.md  SECURITY.md  LICENSE  README.md
```

Code contract between UI and backend: Rust types are exported to TypeScript with `ts-rs` or `specta`, so a renamed field breaks the TypeScript build instead of failing at runtime. Long-running work streams to the UI as Tauri events (`transaction://progress`, `transaction://output`, `transaction://prompt`).

## Roadmap and milestones

A public 0.1 alpha by the end of January 2027 and 1.0 in June 2027 is realistic for one part-time developer if scope stays fixed; each release is usable on its own.

&#91;embedded content: roadmap · 6 phases to 1.0\]

Every phase ends with a tagged release in the `-git` AUR package; 0.1 onwards also gets a GitHub Release.

| Milestone | Scope | Exit criteria |
| --- | --- | --- |
| Spikes and decisions | PTY yay runner, AUR client, Lua review-pin prototype, Tauri vs Wails | ADR written; a small AUR package installs from a test program with the sudo prompt handled |
| Foundations | Monorepo, CI, Arch test container, design tokens, search screen | Search across repos + AUR returns in under 300 ms (repos) on a test machine |
| 0.1 alpha: core flows | Package detail, queue, PKGBUILD review, install/remove, terminal drawer | Install and remove repo and AUR packages end to end in CI; first outside testers |
| 0.2: updates | Updates dashboard, full upgrade flow, devel packages, orphan cleanup, Arch News check | Weekly `-Syu` done through the app on your own machine for a month without falling back to the terminal |
| 0.3 beta: safety | Diff-since-last-install, review pinning, risk signals, maintainer-change alerts, tray notifier, cache manager | A changed commit after review aborts the build in CI; defanged malicious samples are flagged |
| 0.4 RC: polish | yay/Lua settings editor, i18n (English + Bengali first), accessibility pass, docs site | No open P1 bugs; translations at 100% for two languages |
| 1.0 release | AUR `-bin` and source packages, announcement | Success criteria in Vision and goals met |

## Testing and CI

The riskiest code is the part that talks to yay and pacman, so most test effort goes there — against recorded output and disposable Arch containers, never the developer's real system.

| Layer | What is tested | How |
| --- | --- | --- |
| Unit (Rust) | Transaction planner, safety rules (no partial upgrade, no root), name validation, risk heuristics | `cargo test`, property tests with `proptest` |
| Parser | yay/pacman output parsing, prompt detection, error classification | Golden files: recorded real yay sessions in `tests/fixtures`, re-recorded on each yay release |
| AUR client | Search/info, batching under the URL-length limit, rate limiting, caching | Mock HTTP server (`wiremock`); one nightly live smoke test |
| Integration | Real installs, upgrades, removals, AUR builds, review-pinning abort | `archlinux:base-devel` Docker container in CI with a non-root user and yay installed; small known AUR packages |
| Security regression | Defanged copies of 2026-style malicious PKGBUILDs trigger risk highlights; changed commit after review aborts the build | Integration suite |
| UI | Components, flows (queue → review → run), accessibility | Vitest + Testing Library; Playwright against the Tauri dev build; axe checks |

**CI (GitHub Actions)**

- On every PR: format + lint (`cargo fmt`, `clippy -D warnings`, Biome), type-check, unit + parser tests, UI tests, `cargo deny`, `pnpm audit`.
- Nightly: full Arch-container integration suite against the latest yay and pacman, so upstream changes are caught within a day.
- On tag: build release artifacts, generate checksums, sign, publish GitHub Release, open a PR updating the AUR PKGBUILD.
- Conventional Commits + `release-please` (or `git-cliff`) for changelogs.

## Packaging and distribution

Ship through the AUR as a native package; skip Flatpak, Snap and AppImage. A sandboxed app cannot reach the host's libalpm, pacman database or yay without punching holes that defeat the sandbox, and Arch users expect AUR delivery.

- **`yaygui`** — builds from the release tarball (needs Rust, Node/pnpm as makedepends).
- **`yaygui-bin`** — prebuilt binary from GitHub Releases, verified by checksum and signature; the fastest install for most users.
- **`yaygui-git`** — tracks `main` for testers.
- **Dependencies:** `yay` (>= 13 for review pinning; degrade gracefully on 12.x), `pacman`, `webkit2gtk-4.1`, `pacman-contrib` (for `checkupdates`, `paccache`), optional `polkit`, `reflector`.
- **Desktop integration:** `.desktop` file, AppStream metainfo XML, hicolor icons, an autostart entry for the notifier (off by default).
- **Later:** ask EndeavourOS, CachyOS and Garuda maintainers about including it in their repos once 1.0 is stable — distro adoption is the biggest growth lever.

Because the package itself lives in the AUR, the project should practise what it preaches: two co-maintainers on the AUR package, signed release tags, and the PKGBUILD kept short and readable.

## Open-source setup

**License: GPL-3.0-or-later.** The app links libalpm (GPL-2.0-or-later) through the `alpm` crate (GPL-3.0), and yay itself is GPL-3.0-or-later, so a GPL-3.0 license is the compatible and expected choice in this ecosystem. Not a lawyer — but this is the standard pattern for Arch tooling.

**Day-one files**

- `README.md` — one-line pitch, screenshot/GIF, install (`yay -S yaygui-bin`), safety principles, roadmap link.
- `CONTRIBUTING.md` — dev setup in under 10 minutes (a `just dev` or `make dev` target), coding style, commit format, how to run the Arch test container.
- `CODE_OF_CONDUCT.md` (Contributor Covenant), `SECURITY.md` (private report email or GitHub private advisories), `LICENSE`.
- Issue templates: bug (with `yay --version`, `pacman --version`, log attachment), feature request, translation; PR template with a safety checklist.
- `docs/adr/` — short architecture decision records (why Tauri, why no root daemon, why no Flatpak) so newcomers see the reasoning.

**Governance**

- Start as BDFL (you), with a stated path to a small maintainer team after 3+ months of consistent contributions from others.
- Two-approval rule for anything touching `yay-runner`, `review`, privilege escalation or the PKGBUILD.
- Label `good first issue` and `help wanted` from week one; translations are the easiest on-ramp (Weblate or Transifex — yay uses Transifex).

**Community and launch**

- Pick a distinct name early (check AUR, GitHub, crates.io, npm) and avoid using "yay" as the brand — e.g. a name that hints at "friendly AUR" — then describe it as "a GUI for yay".
- Announce at 0.1 on r/archlinux, EndeavourOS and CachyOS forums, and Arch BBS "Community Contributions"; lead with the safety story.
- Tell the yay maintainer about the project and the Lua-hook use; upstream feedback and any needed hook fields are worth more than stars.
- GitHub Discussions for Q\&A; a Matrix room once there are regular contributors.
- Funding later via GitHub Sponsors / Open Collective; never bundle telemetry. If usage stats are ever added, opt-in only.

## Risks and mitigations

| Risk | Likelihood | Impact | Mitigation |
| --- | --- | --- | --- |
| yay output or prompts change and break parsing | High | High | Parse as little as possible (use libalpm and AUR RPC for data, yay only for actions); golden-file tests re-recorded per yay release; nightly CI on latest yay; raw terminal always visible as fallback |
| The app is blamed for installing a malicious AUR package | Medium | Very high | Mandatory review, risk signals, review pinning, clear wording that the AUR is unvetted; never show a "safe" badge |
| A bug causes a partial upgrade or broken system | Low | Very high | Planner enforces full-upgrade rule; integration tests in Arch containers; beta channel (`-git`) before releases |
| AUR RPC limit (4,000 requests/day per IP) or URL-length limits hit | Medium | Medium | Batch info calls (\~200 names per request), local cache with TTL, use the AUR metadata dump for browsing ([Aurweb RPC docs](https://wiki.archlinux.org/title/AurJson)) |
| AUR outages or adoption/push freezes during incidents | Medium | Medium | Offline-friendly cache; show AUR status banner; repo features keep working |
| WebKitGTK rendering or performance issues on some GPUs | Medium | Medium | Document env-var workarounds; keep the UI light; virtualised lists |
| Rust learning curve slows you down | Medium | Medium | Keep the backend thin; start with `yay-runner` + `aur-client` only; Wails/Go fallback decided by end of month 1 |
| Solo-maintainer burnout | High | High | Small scope per release, recruit co-maintainers early, say no to feature creep (Flatpak/Snap support is out of scope) |
| Name or trademark confusion with yay | Low | Medium | Distinct project name; "unofficial" in the README; ask the yay maintainer's blessing |

## Immediate next steps

The first two weeks are about proving the riskiest piece — driving yay from a PTY and pinning a review — before building any polished UI.

- [ ] Pick the project name; reserve the GitHub org, AUR package names and crates.io names.
- [ ] Set up an Arch VM or container (Distrobox/Docker `archlinux:base-devel`) as the only place the app touches pacman during development.
- [ ] Read yay's Lua hook documentation and confirm the event fields for `AURPreInstall`; prototype the overlay `init.lua` that aborts on a commit mismatch.
- [ ] Spike: Rust program that runs `yay -S <small AUR package>` in a PTY, streams output, detects the sudo prompt and answers it.
- [ ] Spike: AUR RPC v5 search + info client with batching and caching.
- [ ] Scaffold the Tauri 2 + React + TS app; one screen that searches repos + AUR and shows results.
- [ ] Decide Tauri vs Wails at the end of the spikes and write it up as the first ADR.
- [ ] Write README, CONTRIBUTING, SECURITY, CODE\_OF\_CONDUCT, LICENSE (GPL-3.0-or-later); make the repo public at the first working spike.
- [ ] Message the yay maintainer about the project and the hook-based review pinning.
