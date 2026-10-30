# Contributing to YayGUI

Thanks for helping. YayGUI is pre-alpha, so the best contributions right now are small, well-tested changes, bug reports with logs, and translations once the catalogs exist.

## Ground rules

- Be kind. Participation is covered by the [Code of Conduct](CODE_OF_CONDUCT.md).
- Discuss large changes in an issue before writing code.
- Security problems go through [SECURITY.md](SECURITY.md), not public issues.
- Every change must respect the safety rules in the [README](README.md#safety-principles): never run as root, no partial upgrades, no silent AUR builds, no stored credentials, no shell strings.

## Development setup

You need:

- Rust stable, 1.85 or newer (the workspace uses edition 2024)
- Node.js 22 and pnpm (for the UI, once `apps/desktop` is in the repository)
- Git

Clone the repository, then:

```sh
cargo test --workspace
```

For the UI:

```sh
cd apps/desktop
pnpm install
pnpm dev
```

## Testing against pacman: use an Arch container

Never run code that installs, upgrades or removes packages on your everyday system. Use a disposable Arch container, which is also what CI uses.

```sh
docker run -it --rm -v "$PWD":/work -w /work archlinux:base-devel bash
```

Inside the container, create a non-root user. `makepkg` refuses to run as root, and YayGUI must never run as root either:

```sh
pacman -Syu --noconfirm sudo git
useradd -m dev
echo 'dev ALL=(ALL) ALL' > /etc/sudoers.d/dev
su - dev
```

Then build and test as `dev`, from `/work`. Install yay in the container when you need to exercise real `yay` calls. Do not run the app's write paths against the host.

## Coding style

Rust:

- `cargo fmt --all` before committing. CI runs `cargo fmt --all --check`.
- `cargo clippy --workspace --all-targets -- -D warnings` must pass. Warnings are errors.
- `unsafe` code is forbidden in the workspace.
- Pass commands as argument arrays. Never build a command with `sh -c` or string concatenation.
- Validate package names against the pacman/AUR name pattern before use.

TypeScript (`apps/desktop`):

- [Biome](https://biomejs.dev/) handles formatting and linting. Run `pnpm lint` before pushing.
- `pnpm typecheck`, `pnpm test` and `pnpm build` must pass.
- Use the typed wrappers in `src/api/` to call the backend. Do not call Tauri commands directly from feature code.

## Commits and pull requests

Use [Conventional Commits](https://www.conventionalcommits.org/), for example:

```
feat(aur-client): batch info requests under the URL length limit
fix(yay-runner): detect sudo prompt split across PTY reads
docs(adr): add ADR for read/write split
```

Common types: `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `build`, `ci`, `chore`. Use `!` or a `BREAKING CHANGE:` footer for breaking changes.

Open pull requests against `main`. Fill in the pull request template, including the safety checklist. Keep PRs focused, and add tests for behaviour changes.

## Two-approval rule

Changes to any of these need two approving reviews from maintainers before merge:

- `crates/yay-runner`
- review logic (`crates/review` and PKGBUILD or diff handling)
- privilege escalation (sudo, polkit, or any code that asks for a password)
- anything that changes what is built or installed, including the PKGBUILD for the app itself

## Safety checklist

Before you open a PR, confirm:

- [ ] The app still refuses to run as root, and nothing you added runs as root.
- [ ] No code path performs a partial upgrade, and `pacman -Sy` is not used for update checks.
- [ ] No AUR build happens without showing the PKGBUILD or diff for review, and the review cannot be skipped silently.
- [ ] No command is built as a shell string. Package names are validated.
- [ ] No credentials are written to disk, logs or the database.
- [ ] Tests cover the new behaviour, including any safety rule it touches.

## Translations

Translations are a great first contribution. Catalogs will live in `apps/desktop/src/i18n/` as JSON. Open a translation issue (the template is in the issue chooser) to coordinate a language before you start.

## Questions

Use GitHub Discussions for questions, and open an issue for bugs and feature requests.
