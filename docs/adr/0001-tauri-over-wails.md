# 0001: Use Tauri 2 rather than Wails for the desktop shell

- Status: Proposed. To be confirmed at the end of the spikes (see PLAN.md, "Spikes and decisions").
- Date: 2026-10-27
- Deciders: project maintainer

## Context and problem statement

YayGUI needs a desktop shell with a modern web-based UI and a backend that talks to libalpm, the AUR, a pseudo-terminal and D-Bus. The choice is between Tauri 2 (Rust backend) and Wails (Go backend). Native toolkits (GTK 4, Qt 6) and Electron were also considered and rejected for reasons below.

## Decision drivers

- Small footprint: use the system WebKitGTK rather than bundling Chromium.
- Strict permissions: a capability model that limits what the frontend can call.
- Access to mature libraries for libalpm, PTYs (`portable-pty`) and async work.
- The team's existing React and TypeScript skills for the UI.
- Maintainability by Arch users who may want to contribute.

## Considered options

- **Tauri 2** with React, TypeScript and Rust. Strong capability model, small binaries, Rust bindings for alpm exist.
- **Wails** with React, TypeScript and Go. Easier backend language, and yay's own Go libraries (go-alpm, Jguer/aur) could be reused. Smaller ecosystem and fewer plugins.
- **GTK 4 with libadwaita.** Truly native on GNOME, but a new toolkit to learn and foreign-looking on KDE.
- **Qt 6 and QML.** Native on KDE, but C++ or complex bindings, and licensing questions for some contributors.
- **Electron.** Fastest to start, but a 150 MB+ install and high memory use. Arch users are expected to reject it.

## Decision outcome

Proposed: **Tauri 2**, because it gives the best balance of footprint, permission model and UI productivity. This is provisional. The spikes (PTY runner, AUR client, Lua review-pin prototype) will test the Rust learning curve and WebKitGTK behaviour. If they fail the bar, Wails with Go is the fallback, and this record will be updated.

### Consequences

- Good: small binaries, a restrictive capability model, one Rust `core` crate that is testable without the UI.
- Good: the UI can be built with existing web skills.
- Bad: a Rust learning curve for contributors.
- Bad: WebKitGTK rendering quirks on some GPUs, which need documented workarounds.
- Bad: the UI will look web-styled rather than pixel-native, so theming must follow the system theme.

## More information

- PLAN.md, "Tech stack" and "Risks and mitigations" (Rust learning curve, WebKitGTK issues).
