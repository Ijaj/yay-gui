# Security Policy

## Reporting a vulnerability

Please do **not** open a public issue for security problems.

Report them privately through GitHub's private vulnerability reporting: open the [new security advisory form](https://github.com/ijaj/yay-gui/security/advisories/new) for this repository. Include:

- a description of the problem and its impact
- steps to reproduce, ideally in an Arch container
- the affected version or commit
- the output of `yay --version` and `pacman --version`, if relevant

Maintainers will acknowledge reports as soon as they can, keep you informed while the issue is investigated, and credit you in the advisory if you wish.

## Supported versions

YayGUI is pre-alpha. Only the latest commit on `main` receives security fixes until the first tagged release.

## Scope

In scope, for this project's own code:

- running as root, or any path that makes YayGUI or yay build or install as root
- partial upgrades, or any path that installs repository packages without a full `-Syu`
- bypassing the PKGBUILD or diff review, including building an AUR package that differs from the approved version
- shell injection, or any command built from untrusted strings
- leaking or persisting sudo passwords or other credentials
- Tauri capability or webview issues that let remote or untrusted content reach the backend

Out of scope:

- malicious or vulnerable content inside AUR packages themselves. Report those to the Arch Linux security team and the package maintainer.
- vulnerabilities in yay, pacman, libalpm, makepkg or WebKitGTK. Report those upstream. We are happy to be told about them if they affect YayGUI's design.
- issues that need an attacker who already has root on the machine.

## Supply chain

Release tags are intended to be signed, and CI runs `cargo deny` and `pnpm audit`. Reports about the release or packaging process are in scope.
