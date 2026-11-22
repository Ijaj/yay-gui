# Arch container harness

Runs aurdeck's install and remove path against real pacman and real yay, in a disposable Arch container. Nothing here ever touches the host's packages.

```sh
tests/arch/run.sh                      # all scenarios
tests/arch/run.sh repo-install         # or aur-install, remove
tests/arch/run.sh aur-install --no-overlay
```

`run.sh` builds `aurdeck-arch-harness` (crate `crates/arch-harness`) on the host, builds the image from `Dockerfile`, and runs the binary inside it as the non-root user `dev`. It needs docker and a Rust toolchain.

## Image

- Base `archlinux:base-devel`, full `pacman -Syu` at build time.
- User `dev` with a password; `sudo` always asks for it (`timestamp_timeout=0`, no `NOPASSWD` in the final image). The password is the throwaway `AURDECK_HARNESS_SUDO_PASSWORD` set in the Dockerfile; the harness answers sudo prompts with it through `TxHandle::answer_password`.
- yay from the AUR (`yay-bin`), built as `dev`. The version is logged at build time and stored in `/etc/aurdeck-yay-version`; `run.sh` prints it. Current: **yay v13.1.0 - libalpm v16.0.1** (built Nov 15, 2026).
- `AURDECK_ARCH_HARNESS=1` is set only here. The binary refuses to start unless that variable is `1`, `/.dockerenv` or `/run/.containerenv` exists, and the uid is not 0.

## Scenarios

Each one goes through the same path as the desktop app: `PlanContext` from a real `ReviewStore` (temp dir), AUR packages fetched with `AurGit` and approved at the fetched commit, a review-pin `Overlay` passed through `TxConfig.env`, commands from `aurdeck_core::plan`, run with `start_transaction`. Prompts are answered from the table in `crates/arch-harness/src/prompts.rs`; an unrecognised prompt (no output for 120 s) or a 10-minute timeout fails the scenario. Post-conditions are read from the real local pacman database with `aurdeck-alpm-reader`.

| Scenario | What it does | Post-condition |
| --- | --- | --- |
| `repo-install` | `yay -Syu --needed -- tree` (the planner requires a full upgrade with repo installs) | `tree` installed |
| `aur-install` | fetch and approve `downgrade`, `yay -S --needed --answerdiff None --answeredit None -- downgrade` with the overlay | `downgrade` installed |
| `remove` | `yay -Rs -- tree` (installs it first if absent) | `tree` absent |

`--no-overlay` on `aur-install` builds without the review-pin overlay. It is a diagnostic only, to separate "the build works" from "the overlay works"; the real scenario keeps the overlay.

## Test packages

- Repo: `tree` (extra). A few dozen KiB, depends only on glibc, nothing else to download.
- AUR: `downgrade`. `arch=('any')`, a bash script; depends on the repo packages `pacman-contrib` and `fzf` (installed by yay as dependencies), one source tarball from a `github.com` release, builds in about a second. It is its own pkgbase and nothing else provides its name. The scenario refuses to run if the name ever appears in a sync repo. Two candidates were dropped: `pfetch` resolves to `extra/pfetch-rs` since 2026, so yay installed the repo package; `pkgbuild-introspection` is gone from the AUR (only `-git`, built from a `git://` GitHub clone the proxy refuses, was offered), and yay silently built that instead of the package the harness had fetched and approved.

## Network

Inside the cloud agent container, traffic goes through a proxy. `run.sh` passes the proxy variables and adds `/root/.ccr/ca-bundle.crt` as a trust anchor only when `HTTPS_PROXY` is set (override the CA path with `AURDECK_PROXY_CA`), so the same script works in GitHub Actions and on a machine without a proxy. The image's sudoers keeps the proxy variables so pacman run through sudo can reach the mirrors.

## Known gaps found by this harness

- `PromptDetector` does not report yay's `==> Packages to exclude:` menu (the options line is ` -> Excluding packages may cause partial upgrades`, not `[N]one`). The harness answers it from a screen-tail table (`RAW_MENUS`); remove the entry when the detector learns the prompt. The desktop app has the same blind spot.
