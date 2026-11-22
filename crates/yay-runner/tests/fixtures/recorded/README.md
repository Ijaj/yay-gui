# Recorded yay sessions

Raw PTY bytes of real `yay` runs, unmodified (no redaction, no hand edits). Used by the detector
tests in `src/detector.rs` (`recorded_*`).

- yay v13.1.0 (`yay-bin` 13.1.0-1 from the AUR), libalpm v16.0.1, pacman 7.1.0, sudo 1.9.17p2
- Recorded Nov 15 and Nov 22, 2026 in a throwaway `archlinux:base-devel` container (`--network host`
  through the cloud agent proxy), as user `alice` (non-root) with a sudo password. The password
  `hunter2` of this disposable user is typed into the PTY; it is never echoed, so it is not in any file.
- Terminal: 120 columns x 40 rows, `TERM=xterm-256color`, `LANG=C.UTF-8`.
- Recorder: `record.py` in this directory (refuses to run outside a container or as root; exits non-zero on a timeout, except `"expect_timeout": true` as used for `db_lock`, or on an unmatched step) (a `pty.fork` that logs every byte read from the PTY master
  and answers prompts from a list of regexes). Typed answers appear in the recording only as the
  terminal echo. Default `/etc/pacman.conf` (no `Color`), except `repo_install_color`.
- The AUR package is `pfetch-git` (no dependencies; source is a git clone from github.com). The plain
  `pfetch` resolves to `extra/pfetch-rs` and its AUR tarball source is refused by the proxy.
- Sandbox-only setup, not part of the recordings: `Defaults env_keep += "HTTPS_PROXY ..."` in sudoers so
  pacman can download through the proxy. The recordings contain systemd's `OSC 3008` context sequences
  (`ESC ] 3008;start=...`) emitted by sudo; they are real and the detector must ignore them.

| File | Command (as alice) | What it shows |
|---|---|---|
| `repo_install.bin` | `yay -S --needed cowsay` | sudo prompt, `Proceed with installation? [Y/n]` (answered Enter) |
| `repo_install_color.bin` | `yay --color always -S --needed cowsay` | same with ANSI colour on prompts |
| `wrong_password.bin` | `yay -S --needed sl` | wrong sudo password then the right one, `Sorry, try again.`, confirmation answered `n` |
| `wrong_password_3x.bin` | `yay -S --needed sl` | three wrong passwords: `sudo: 3 incorrect password attempts` |
| `target_not_found.bin` | `yay -S --needed aurdeck-no-such-package-xyz` | yay's `No AUR package found for ...` |
| `remove_not_found.bin` | `yay -R aurdeck-no-such-package-xyz` | sudo prompt, pacman `error: target not found: ...` |
| `db_lock.bin` | `yay -S --needed sl` while a root `pacman -S sl` waits at its confirmation (holds `db.lck`) | yay's `db.lck is present` / `There may be another Pacman instance running. Waiting...`; the recorder killed yay after 25 s, so the last line has no newline |
| `aur_install_menus.bin` | `yay -S --needed --editmenu aur/pfetch-git` (fresh cache) | clean-build menu (`N`), diff menu (`A`), `Proceed with install? [Y/n]` after the diff, edit menu (`N`), then makepkg output, sudo, `Proceed with installation?` |
| `aur_install_app.bin` | `yay -S --needed --answerdiff None --answeredit None -- pfetch-git` | the app's flags. The AUR repo was cloned first with `git clone` into `~/.cache/yay/` (as the GUI review does); the clean menu is answered `N` |
| `remove.bin` | `yay -R -- pfetch-git` | sudo prompt, `Do you want to remove these packages? [Y/n]` |
| `syu_repo.bin` | `yay -Syu --needed -- tree` | sudo prompt (database refresh), the `==> Packages to exclude:` menu (Enter), `Proceed with installation?`, full upgrade of 11 packages |
| `syu_aur.bin` | `yay -Syu --needed --answerdiff None --answeredit None -- pfetch-git` | exclude menu, then the AUR clean and diff menus, build, install |

Observations the detector tests rely on:

- yay prints the clean-build and diff menus even with `--answerdiff None`; the flag answers the diff
  menu itself (`==> None` follows the bare `==> `). With `--answeredit None` there is no edit menu.
  Consumers must not answer a menu that yay already answered.
- The `Packages to exclude` menu has a ` -> Excluding packages may cause partial upgrades...` line
  between its heading and the bare `==> ` prompt.
- makepkg colour sequences use `ESC ( B` (three bytes).
- Consumer rules, via `PromptEvent::menu_kind()` (`MenuKind`): answer `UpgradeExclude` (`-Syu`) only
  with an empty line or a repository name such as `aur`, never with package names (excluding a package
  causes a partial upgrade). Ignore `CleanBuild`, `Diffs` and `Edit` menus when the matching
  `--answerclean` / `--answerdiff` / `--answeredit` flag was passed: yay answers them itself.
- Prompts answered on the same line (`[Y/n] n`, `==> None`) are not reported. Only a prompt that
  arrives without its answer is (the live path); the tests feed the recordings that way (cut after each
  prompt). A prompt line that ends in a newline with an empty answer (Enter) is still reported when a
  whole recording is fed at once.
- yay waits on the lock (`Waiting...`) on the same line instead of failing.

## Hand-written fixtures (synthetic)

`install_session.txt`, `menus.txt` and `errors.txt` in the parent directory are NOT recorded. They
stay because the recordings do not cover every kind they assert: the `Replace foo-old with extra/foo?
[y/N]` conflict prompt, `error: failed to commit transaction (conflicting files)`, and a makepkg
`A failure occurred in build()` / `Error making:` failure. Treat them as synthetic until recorded.

## Re-recording

In a disposable container as a non-root user: write a JSON scenario (see the docstring of
`record.py`) and run `python3 record.py scenario.json`. For the lock recording start
`sleep 120 | pacman -S sl` as root first. Clear `~/.cache/yay/*` and remove installed test packages
between runs. `faillock --user alice --reset` is needed after wrong-password runs.
