# Arch container harness

Runs aurdeck's install and remove path against real pacman and real yay, in a disposable Arch container. Nothing here ever touches the host's packages.

```sh
tests/arch/run.sh                      # all scenarios
tests/arch/run.sh repo-install         # or aur-install, remove
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

Each one goes through the same path as the desktop app: `PlanContext` from a real `ReviewStore` (temp dir), AUR packages fetched with `AurGit` and approved at the fetched commit, a review-pin `Overlay` passed through `TxConfig.env`, commands from `aurdeck_core::plan`, run with `start_transaction`. Prompts are answered from the table in `crates/arch-harness/src/prompts.rs`; an unrecognised prompt (any other bare `==>` menu, or no output for 120 s) or a 10-minute timeout fails the scenario; after a cancel the harness exits non-zero if the transaction does not stop within 15 s. Post-conditions are read from the real local pacman database with `aurdeck-alpm-reader`.

| Scenario | What it does | Post-condition |
| --- | --- | --- |
| `repo-install` | `yay -Syu --needed -- tree` (the planner requires a full upgrade with repo installs) | `tree` installed; no AUR or foreign package was built or changed |
| `aur-install` | fetch and approve `downgrade`, `yay -S --needed --answerdiff None --answeredit None -- downgrade` with the overlay | `downgrade` installed, it is the only foreign package added, and the commit yay built (HEAD of `~/.cache/yay/downgrade`) equals the approved commit |
| `remove` | `yay -Rs -- tree` (installs it first if absent) | `tree` absent; no foreign package changed |

`--no-overlay` on `aur-install` builds without the review-pin overlay. It is a diagnostic only, to separate "the build works" from "the overlay works"; the real scenario keeps the overlay. The wrong-pin negative case (approved commit differs from what yay would build, the build must abort) needs a working overlay and belongs to P0.5.

## Test packages

- Repo: `tree` (extra). A few dozen KiB, depends only on glibc, nothing else to download.
- AUR: `downgrade`. `arch=('any')`, a bash script; depends on the repo packages `pacman-contrib` and `fzf` (installed by yay as dependencies), one source tarball from a `github.com` release, builds in about a second. It is its own pkgbase and nothing else provides its name. The scenario refuses to run if the name ever appears in a sync repo. Two candidates were dropped: `pfetch` resolves to `extra/pfetch-rs` since 2026, so yay installed the repo package; `pkgbuild-introspection` is gone from the AUR (only `-git`, built from a `git://` GitHub clone the proxy refuses, was offered), and yay silently built that instead of the package the harness had fetched and approved.

## Network

Inside the cloud agent container, traffic goes through a proxy. `run.sh` passes the proxy variables and adds `/root/.ccr/ca-bundle.crt` as a trust anchor only when `HTTPS_PROXY` is set (override the CA path with `AURDECK_PROXY_CA`), so the same script works in GitHub Actions and on a machine without a proxy. The image's sudoers keeps the proxy variables so pacman run through sudo can reach the mirrors. With a proxy the container uses `--network host`, so it runs with `--cap-drop ALL` plus only CHOWN, DAC_OVERRIDE, FOWNER, FSETID, KILL, SETFCAP, SETGID, SETPCAP, SETUID, SYS_CHROOT and AUDIT_WRITE (no NET_RAW: a hostile PKGBUILD that sudos with the known password must not sniff the host's interfaces). The binary is found through `cargo metadata`, so `CARGO_TARGET_DIR` is honoured.

## Known gaps found by this harness

- `PromptDetector` does not report yay's `==> Packages to exclude:` menu (its options line is ` -> Excluding packages may cause partial upgrades`, not `[N]one`), which appears on every `yay -Syu` with something to install. The harness (`src/screen.rs`) answers it only when the last non-empty line of the screen is that menu's own bare `==>` prompt, at most once per command, and fails on any other bare `==>` menu. Remove this when the detector learns the prompt. The desktop app has the same blind spot.
- The answer is `aur`, not empty. Verified on yay v13.1.0 with an outdated AUR package installed: `--answerupgrade aur` (same parser as the menu) printed "there is nothing to do" and left it at the old version, while an empty answer upgraded and rebuilt it. An empty answer would let a repo scenario build unreviewed AUR updates. The planner's `-Syu` pulling in AUR updates in the app is a separate issue for the planner's owner.
