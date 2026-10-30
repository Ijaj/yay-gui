## Summary

<!-- What does this change, and why? Link the issue if there is one. -->

## Type of change

- [ ] Bug fix
- [ ] New feature
- [ ] Refactor, performance or tests only
- [ ] Documentation or build
- [ ] Breaking change

## Safety checklist

- [ ] The app does not run as root, and this change does not make anything build or install as root.
- [ ] No partial upgrade is possible through this change, and `pacman -Sy` is not used for update checks.
- [ ] No AUR build can happen without the PKGBUILD or diff review, and the review is not bypassed.
- [ ] No shell string is built. Commands use argument arrays, and package names are validated.
- [ ] No credentials are stored on disk, in logs or in the database.
- [ ] Tests are added or updated for the behaviour changed, including any safety rule it touches.

## Two-approval rule

- [ ] This PR does not touch `crates/yay-runner`, review logic, privilege escalation or the PKGBUILD. If it does, it needs two approving reviews.

## Testing

<!-- How did you test this? For anything touching pacman or yay, say that you used the Arch container. -->

- [ ] `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass locally
- [ ] UI changes: `pnpm lint`, `pnpm typecheck`, `pnpm test` and `pnpm build` pass

## Notes for reviewers

<!-- Anything you want the reviewer to look at first. -->
