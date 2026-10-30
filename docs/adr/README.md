# Architecture decision records

This folder records significant architecture decisions for YayGUI, in a lightweight [MADR](https://adr.github.io/madr/)-style format. Each ADR explains the context, the decision and its consequences, so newcomers can see why the project is shaped the way it is.

Start from [0000-template.md](0000-template.md) for a new record. Number records sequentially and never reuse a number. To change a decision, write a new ADR that supersedes the old one and update the old status.

## Index

| ADR | Title | Status |
| --- | --- | --- |
| [0001](0001-tauri-over-wails.md) | Use Tauri 2 rather than Wails for the desktop shell | Proposed |
| [0002](0002-no-root-daemon.md) | No root daemon or setuid helper | Accepted |
| [0003](0003-no-flatpak.md) | Distribute through the AUR, not Flatpak, Snap or AppImage | Accepted |
| [0004](0004-reads-via-alpm-writes-via-yay.md) | Read data through libalpm and the AUR RPC; make every change through yay | Accepted |

## Statuses

- **Proposed:** under discussion, not yet final.
- **Accepted:** agreed and in force.
- **Superseded by NNNN:** replaced by a later record.
- **Deprecated:** no longer applies, and nothing replaces it.
