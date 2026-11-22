# Arch container harness

Runs aurdeck's install and remove path against real pacman and real yay, in a disposable Arch container. Nothing here ever touches the host's packages.

```sh
tests/arch/run.sh                      # all scenarios
tests/arch/run.sh repo-install         # or aur-install, pin-commit-mismatch, pin-pkgbuild-mismatch, pin-unapproved-base, pin-unreviewed-upgrade, remove
tests/arch/run.sh aur-install --no-overlay
```

`run.sh` builds `aurdeck-arch-harness` (crate `crates/arch-harness`) on the host, builds the image from `Dockerfile`, and runs the binary inside it as the non-root user `dev`. It needs docker and a Rust toolchain.

## Image

- Base `archlinux:base-devel`, full `pacman -Syu` at build time. `run.sh` builds with `--pull` and a date build arg, so a cached `-Syu` layer is never reused across days; `AURDECK_ARCH_REUSE_IMAGE=1` skips the build only for an image younger than a day.
- User `dev` with a password; `sudo` always asks for it (`timestamp_timeout=0`, no `NOPASSWD` in the final image). The password is a throwaway kept in `/etc/aurdeck-harness-sudo-password` (owner `dev`, mode 0400), not in the image environment. The harness reads it once into a `Zeroizing<String>` and answers sudo prompts with it through `TxHandle::answer_password`; yay, makepkg and PKGBUILDs do not inherit it in their environment.
- yay from the AUR (`yay-bin`), built as `dev`. The version is logged at build time and stored in `/etc/aurdeck-yay-version`; `run.sh` prints it. Current: **yay v13.1.0 - libalpm v16.0.1** (built Nov 15, 2026); the yay-bin AUR commit is logged at build time and stored in `/etc/aurdeck-yay-bin-commit`.
- The binary refuses to start unless all of these hold: `AURDECK_ARCH_HARNESS=1` (set only by the Dockerfile), `/.dockerenv` or `/run/.containerenv` exists, the root-owned marker `/etc/aurdeck-arch-harness-image` exists (so a distrobox or toolbox on an Arch host does not pass), the uid is not 0, the user is `dev` and `$HOME` is `/home/dev`.
- The image is local only: built behind a proxy it carries the proxy CA in its trust store and the throwaway sudo password, so never push it to a registry.

## Scenarios

Each one goes through the same path as the desktop app: `PlanContext` from a real `ReviewStore` (temp dir), AUR packages fetched with `AurGit` and approved at the fetched commit, a review-pin `Overlay` (commit and PKGBUILD text per approved base) passed through `TxConfig.env`, commands from `aurdeck_core::plan`, run with `start_transaction`. Prompts are answered from the table in `crates/arch-harness/src/prompts.rs`: yay's menus arrive as `PromptEvent::Menu` and are classified with `menu_kind()` (the exclude menu gets `aur`, or empty in `pin-unreviewed-upgrade`; `CleanBuild` gets `N`; `Diffs`/`Edit` are ignored only when the plan passed `--answerdiff`/`--answeredit`); an unrecognised prompt (any other menu, or no output for 120 s) or a 10-minute timeout fails the scenario; after a cancel the harness exits non-zero if the transaction does not stop within 15 s. Post-conditions are read from the real local pacman database with `aurdeck-alpm-reader`.

| Scenario | What it does | Post-condition |
| --- | --- | --- |
| `repo-install` | `yay -Syu --needed -- tree` (the planner requires a full upgrade with repo installs) | `tree` installed; no AUR or foreign package was built or changed |
| `aur-install` | fetch and approve `downgrade`, `yay -S --needed --answerdiff None --answeredit None -- downgrade` with the overlay | `downgrade` installed, it is the only foreign package added, and the commit yay built (HEAD of `~/.cache/yay/downgrade`) equals the approved commit |
| `pin-commit-mismatch` | approve the commit before HEAD of `downgrade` (with its PKGBUILD) while yay fetches HEAD | transaction fails, yay prints `aurdeck: downgrade changed since review ...`, nothing is built, `downgrade` absent |
| `pin-pkgbuild-mismatch` | approve the correct HEAD commit of `downgrade` but a modified PKGBUILD text through `create_pinned` | yay prints `aurdeck: downgrade: the PKGBUILD differs from the reviewed one ...`, nothing is built, `downgrade` absent |\n| `pin-unapproved-base` | queue `downgrade` with a forged `Approved` in the plan context but an empty approved table in the overlay | yay prints `aurdeck: downgrade was not reviewed in aurdeck ...`, `downgrade` absent (the overlay alone) |
| `pin-unreviewed-upgrade` | install the previous `downgrade` commit with makepkg as `dev` (setup), then (after checking the old version is lower than the AUR's with `vercmp`) `yay -Syu --needed ... -- pacman-cleanup-hook` with only `pacman-cleanup-hook` approved, exclude menu answered empty so the pending AUR upgrade is attempted | yay aborts on `downgrade` (`aurdeck: ... was not reviewed`) before building anything; `downgrade` still at the old version, `pacman-cleanup-hook` absent; `downgrade` (and `pacman-cleanup-hook` if present) removed afterwards |
| `remove` | `yay -Rs -- tree` (installs it first if absent) | `tree` absent; no foreign package changed |

`--no-overlay` on `aur-install` builds without the review-pin overlay. It is a diagnostic only, to separate "the build works" from "the overlay works"; the real scenario keeps the overlay. The pin scenarios cover the negative cases (see the table); `all` runs them before `aur-install`, which needs `downgrade` absent.

## Test packages

- Repo: `tree` (extra). A few dozen KiB, depends only on glibc, nothing else to download.
- Second AUR package (`pin-unreviewed-upgrade`): `pacman-cleanup-hook`, `arch=(any)`, one local source file, depends on `pacman-contrib` only.
- AUR: `downgrade`. `arch=('any')`, a bash script; depends on the repo packages `pacman-contrib` and `fzf` (installed by yay as dependencies), one source tarball from a `github.com` release, builds in about a second. It is its own pkgbase and nothing else provides its name. The scenario refuses to run if the name ever appears in a sync repo. Two candidates were dropped: `pfetch` resolves to `extra/pfetch-rs` since 2026, so yay installed the repo package; `pkgbuild-introspection` is gone from the AUR (only `-git`, built from a `git://` GitHub clone the proxy refuses, was offered), and yay silently built that instead of the package the harness had fetched and approved.

## Network

Inside the cloud agent container, traffic goes through a proxy. `run.sh` passes the proxy variables and adds `/root/.ccr/ca-bundle.crt` as a trust anchor only when `HTTPS_PROXY` is set (override the CA path with `AURDECK_PROXY_CA`), so the same script works in GitHub Actions and on a machine without a proxy. The image's sudoers keeps the proxy variables so pacman run through sudo can reach the mirrors. With a proxy the container uses `--network host`, so it runs with `--cap-drop ALL` plus only CHOWN, DAC_OVERRIDE, FOWNER, FSETID, KILL, SETFCAP, SETGID, SETPCAP, SETUID, SYS_CHROOT and AUDIT_WRITE (no NET_RAW: a hostile PKGBUILD that sudos with the known password must not sniff the host's interfaces). The binary is found through `cargo metadata`, so `CARGO_TARGET_DIR` is honoured.

## Known gaps found by this harness

- In repo scenarios the exclude answer is `aur`, not empty. Verified on yay v13.1.0 with an outdated AUR package installed: `--answerupgrade aur` (same parser as the menu) printed "there is nothing to do" and left it at the old version, while an empty answer upgraded and rebuilt it. An empty answer would let a repo scenario build unreviewed AUR updates. The planner's `-Syu` pulling in AUR updates in the app is a separate issue for the planner's owner.
