# 0003: Distribute through the AUR, not Flatpak, Snap or AppImage

- Status: Accepted
- Date: 2026-10-27
- Deciders: project maintainer

## Context and problem statement

Desktop apps are often shipped as sandboxed bundles. YayGUI, however, manages the host's package database, runs the host's pacman and yay, and needs the host's libalpm and pacman lock. The packaging format must suit that job.

## Decision drivers

- The app must reach the host's libalpm, `/var/lib/pacman` and yay.
- Arch users expect software from the AUR and the official repositories.
- Sandbox holes would undermine the safety model.
- Maintainer time is limited.

## Considered options

- **Flatpak, Snap or AppImage** with the sandbox partly disabled, or with host access punched through.
- **AUR packages** (`yaygui`, `yaygui-bin`, `yaygui-git`), built natively against the host libraries.

## Decision outcome

Chosen option: **AUR packaging only**. A sandboxed app cannot reach the host's libalpm, pacman database or yay without holes that defeat the sandbox. Flatpak, Snap and AppImage are out of scope, and requests for them are declined unless this record is superseded.

### Consequences

- Good: the app works directly with the host's pacman and yay, and the safety model holds.
- Good: packages are native and match how Arch users already install software.
- Bad: users outside Arch-based distributions are not served.
- Bad: the project depends on the AUR for distribution and must keep its PKGBUILDs tidy.

## More information

- PLAN.md, "Packaging and distribution".
