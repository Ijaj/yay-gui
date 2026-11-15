# YayGUI — Completion Plan (agent swarm)

Nov 15, 2026 · covers everything between today's `main` (`52fed85`) and the 1.0 release.

[PLAN.md](../PLAN.md) says **what** to build and why. This document says **what is left, in what order, and how a team of AI agents builds it** so that quality stays high and the token bill stays low. Where the two disagree on scope, PLAN.md wins.

## 1. Where the project stands

| Area | Done | Missing for 1.0 |
| --- | --- | --- |
| Core rules (`crates/core`) | Name validation, planner, safety rules, state machine | Planner cases for AUR updates inside `-Syu`, cache cleaning, ignore list |
| Reading packages (`crates/alpm-reader`) | Local + sync databases (gzip, zstd, xz), reverse deps, version compare | Files list, groups browse; libalpm backend is untested on Arch |
| AUR (`crates/aur-client`) | Search, info, batching, cache, rate limit | Metadata dump for browsing, comments |
| Running yay (`crates/yay-runner`) | PTY runner, prompt detector, lock check, review-pin overlay | **Never run against real yay.** Lua hook API is marked UNVERIFIED in the code. yay version detection |
| Review (`crates/review`) | AUR git fetch, diff, approval store (JSON) | Risk signals, maintainer history, SQLite |
| Desktop app | Search, basic detail, queue, PKGBUILD review, transaction, Installed + removal | Home, Updates, Maintenance, Settings; real package detail; xterm.js terminal; themes; i18n runtime |
| Not started | — | Update checks, Arch News, tray notifier, cache manager, packaging, release pipeline, Arch container tests, Playwright, docs |

Size today: about 6,000 lines of Rust and 3,800 lines of TypeScript. A rough estimate for the rest is 2 to 3 times that.

**The largest technical risk** is still open from PLAN.md's first milestone: nothing has installed a real package through real yay in a test environment, and the review-pinning hook is written against a guessed Lua API. Phase 0 below closes that before any more features are built on top of it.

## 2. Definition of done (1.0)

From PLAN.md, restated as checks an agent or the owner can tick:

1. Daily yay commands work from the UI: `-S`, `-Syu`, `-R`, `-Ss`, `-Si`, `-Qu`, `-Yc`, `-Sc`, `-G`, `-Ps`.
2. The five safety rules hold, each with a test that fails if the rule is broken, and the Arch container suite proves them against real pacman and yay.
3. A commit that changes after review aborts the build (in the container suite).
4. Defanged malicious PKGBUILD samples raise risk highlights.
5. English and Bengali are 100% translated; an accessibility pass has no open blockers.
6. `yaygui`, `yaygui-bin` and `yaygui-git` build from `packaging/aur/`, and a tagged release produces signed artifacts.
7. No open P1 bugs.

Out of scope until after 1.0 (PLAN.md tier "Later"): local PKGBUILD install, rollback hints, mirror ranking, pacman.conf editing, pluggable scanners, AUR voting.

## 3. The swarm

### 3.1 Roles

One orchestrator plans and integrates. It hands narrow, fully specified jobs to cheaper workers, and independent reviewers check the result. No agent reviews its own work.

| Role | Model | Does | Never does |
| --- | --- | --- | --- |
| **Orchestrator** | Top tier (the main session) | Writes briefs and contracts, picks the next wave, resolves review disputes, opens PRs, keeps this plan's checkboxes current | Bulk implementation; re-reading files a worker already summarised |
| **Implementer** | Sonnet | One work package in its own git worktree: code + tests, runs the checks, returns a short report | Touch files outside its brief; change a contract; skip or weaken a test |
| **Mechanical worker** | Haiku | Catalog strings, mock data, fixtures, docs, desktop/metainfo files, renames | Anything in `crates/core`, `crates/yay-runner`, `crates/review` |
| **Reviewer** | Sonnet (standard), top tier or Opus (safety) | Fresh context; sees only the brief, the diff and the safety rules; reports findings ranked by severity | Fix code itself; see the implementer's reasoning |
| **Scout** | Explore agent (read-only) | Answers "where/how is X done" questions for the orchestrator with a conclusion, not file dumps | Write anything |

In Claude Code terms: the `Agent` tool with `model` set per the table and `isolation: "worktree"` for implementers; the `/code-review` and `/security-review` skills for reviewers; `SendMessage` to send review findings back to the same implementer so its context is reused instead of rebuilt. A work package that will run for a long time on its own (the Arch harness, the notifier) can be a separate cloud session instead of an in-process agent.

### 3.2 The loop for every work package

1. **Brief.** The orchestrator fills in `docs/agents/brief-template.md` (created in P0.1): goal, files owned, files to read first (8 at most), the contract (types and command signatures, already written), acceptance tests by name, commands to run, explicit non-goals.
2. **Build.** The implementer works in a worktree and iterates against `scripts/check.sh rust` or `scripts/check.sh ui` until green. It reports in 15 lines or fewer: what changed, test names added, anything it could not do.
3. **Gate 1, machines.** `scripts/check.sh` passes. For packages tagged **arch**, the Arch container suite passes too.
4. **Gate 2, review.** Tier chosen by the table in 3.3. Findings go back to the same implementer. Two fix rounds at most; after that the orchestrator re-scopes the package or takes it over.
5. **Gate 3, integration.** The orchestrator reads the diff stat and the risky hunks, opens a PR that follows the repo's PR template, and updates this plan.
6. **Merge.** The owner merges. Safety-tier PRs always wait for the owner; standard-tier PRs may be merged by the orchestrator only if the owner says so in advance.

### 3.3 Review tiers

The repo's two-approval rule (CONTRIBUTING.md) maps onto the swarm like this:

| Tier | Applies to | Review |
| --- | --- | --- |
| **Standard** | UI screens, read-only backend commands, docs, i18n | One `/code-review` at medium effort by a Sonnet reviewer |
| **Safety** | `crates/yay-runner`, `crates/review`, the planner and safety rules in `crates/core`, anything that builds a command line, privilege escalation, user config files, `packaging/` | Two independent reviews by different agents: `/code-review` at high effort and `/security-review`, both on a top-tier model. Then the owner's approval |

`max` effort reviews are not used: they start many sub-reviewers and cost several times more for little extra on diffs this size.

### 3.4 How the token cost stays down

1. **Contracts first.** The orchestrator writes the Rust DTO and command signature before any worker starts. A backend worker and a UI worker then build against it in parallel and never need to talk. P0.3 makes the TypeScript types generated from Rust, so a mismatch is a compile error, not a review finding.
2. **Tests judge, models don't.** Every brief names its acceptance tests. Workers loop against local checks, which cost no tokens to run. Reviewers only ever see diffs that already pass.
3. **Small, closed context.** A worker reads the files in its brief and one existing example to copy (for a new screen: `installed.rs` + `InstalledScreen.tsx` + its test). Repo conventions live in a short `CLAUDE.md` (P0.1) so no worker rediscovers them.
4. **Cheapest model that can do the job.** Target mix: most implementation tokens on Sonnet, mechanical work on Haiku, top tier only for briefs, integration and safety reviews. Escalate a package to a stronger model only after it has failed on the cheaper one.
5. **Review effort follows risk** (3.3), not habit.
6. **Small packages.** Aim for 300 to 600 changed lines per package. Larger ones are split before they start. Small diffs are cheap to review and rarely need a second round.
7. **No merge conflicts.** Only file-disjoint packages run in the same wave, three or four at a time. P0.4 splits today's shared hot files (`en.json`, `api/types.ts`, `api/mock.ts`, `src-tauri/src/lib.rs`, `state.rs`) by feature so parallel workers stop colliding.
8. **Stop rules.** A worker that fails the same check three times stops and reports. A package that fails review twice goes back to the orchestrator. Nothing loops unattended.
9. **Short reports.** Workers and reviewers return conclusions. The orchestrator does not re-read what they read.
10. **Reuse context.** Fix rounds go to the agent that wrote the code, not a new one.
11. **No speculative work.** "Later" features and anything in section 7 marked *cut first* wait until the gates before them are green.

### 3.5 Ground truth on Arch

This cloud container is Ubuntu: it has no pacman, and its Docker daemon was not running when checked. So real-system verification runs in one of two places:

- **GitHub Actions**, in an `archlinux:base-devel` container, started with `workflow_dispatch` (the orchestrator can trigger runs and read the logs). This uses Actions minutes, which the repo currently saves by keeping CI manual. Budget: one run per **arch**-tagged package plus a nightly run once 0.2 features exist.
- **Docker in the agent's container**, if the daemon can be started there. P0.2 tries this first because it is free; it is unverified.

The owner's own machine is never used for write-path tests (CONTRIBUTING.md forbids it). It is used for the manual acceptance in section 6.

## 4. Work packages

Size: **S** under 300 changed lines, **M** 300 to 600, **L** must be split by the orchestrator before starting. Tier: **Std** or **Safe** (3.3). **arch** means the package must pass the container suite.

### Phase 0 — Make the swarm safe and cheap (sequential, do first)

| ID | Package | Size | Tier | Notes |
| --- | --- | --- | --- | --- |
| P0.1 | `CLAUDE.md` (conventions, commands, safety rules, hot files) and `docs/agents/brief-template.md` | S | Std | Haiku drafts from CONTRIBUTING.md; orchestrator edits |
| P0.2 | **Arch container harness**: `tests/arch/Dockerfile` (non-root user, yay), a headless runner binary that drives `core` + `yay-runner` without Tauri, scenarios for repo install, AUR install, remove; `arch-integration.yml` workflow | M | Safe, arch | Re-records the golden prompt fixtures from real yay. Reports yay's actual Lua hook API |
| P0.3 | Generate TypeScript types from Rust (`specta` or `ts-rs`); CI fails when the generated file is stale | M | Std | Removes the hand-kept "keep in sync" contract in `api/types.ts` |
| P0.4 | Split hot files: per-feature i18n catalogs merged at build, per-feature mock modules, Tauri commands in `commands/<feature>.rs`, `state.rs` into services | M | Std | Pure refactor; every existing test must pass unchanged |
| P0.5 | Fix the review-pin overlay to the real hook API found in P0.2; container test "commit changed after review aborts the build"; yay version detection with graceful fallback on yay 12 (pinning off, UI says so) and a clear screen when yay is missing | M | Safe, arch | If the hook cannot abort or lacks the commit, stop and raise it with the owner (upstream issue) |

P0.1 runs first. P0.2 and P0.3 can run together. P0.4 follows P0.3. P0.5 follows P0.2.

**Exit:** a repo package and an AUR package install and remove through the runner in the container; the pin test passes or the blocker is written up; `scripts/check.sh` is green.

### Phase A — 0.1 alpha: finish the core flows

| ID | Package | Size | Tier | Depends on |
| --- | --- | --- | --- | --- |
| A1 | `package_details` command: repo package (all fields, installed state, required-by) and AUR package (deps, makedeps, optdeps, URL, licence, dates) | M | Std | P0.3 |
| A2 | Package detail screen: Overview, Dependencies (tree + reverse), Files (installed), PKGBUILD link; reachable by URL and from Installed rows | M | Std | A1 contract |
| A3 | Terminal drawer on xterm.js: raw PTY bytes, resize, copy command, keyboard input to prompts | M | Safe | P0.4 (touches the transaction event stream) |
| A4 | Installed: bulk select and bulk removal, links to detail | S | Std | A2 |
| A5 | Interrupted-transaction notice at launch, on any screen (today it only appears when the Transaction screen is opened) | S | Std | — |
| A6 | Container scenarios for the whole queue → review → run flow, including a wrong password and a declined prompt | M | Safe, arch | P0.5 |
| A7 | `packaging/aur/yaygui-git/PKGBUILD`, `.desktop` file, icons; README install section | S | Safe | Owner picks the name first (section 6) |

Wave 1: A1 + A3 + A5. Wave 2: A2 + A6 + A7. Wave 3: A4.

**Exit (PLAN.md):** install and remove repo and AUR packages end to end in CI; tag `0.1.0-alpha`; first outside testers.

### Phase B — 0.2: updates

| ID | Package | Size | Tier | Depends on |
| --- | --- | --- | --- | --- |
| B1 | Update check (`crates/core` or new `crates/updates`): repos through `checkupdates`, never `pacman -Sy`; AUR through info + `vercmp`; devel (`-git`) list; honours IgnorePkg | M | Safe, arch | P0.2 |
| B2 | Planner: `-Syu` that includes AUR updates requires a review per AUR package and pins each one; `--devel` handling | M | Safe | B1 contract |
| B3 | Updates screen: repo / AUR / devel groups, version from → to, sizes, review status, one "Upgrade system" action | M | Std | B1 contract |
| B4 | `crates/news`: Arch News feed, cache, read state; the upgrade is blocked until unread items since the last upgrade are acknowledged | M | Std | — |
| B5 | Home dashboard: update counts, news headlines, orphan count, cache size, last upgrade date, Upgrade button | M | Std | B1, B4 contracts |
| B6 | Container scenario: full upgrade from a dated Arch Linux Archive snapshot, with one AUR update that needs review | M | Safe, arch | B2 |

Wave 1: B1 + B4. Wave 2: B2 + B3 + B5 (UI workers build against mock data). Wave 3: B6.

**Exit (PLAN.md):** the owner does the weekly `-Syu` through the app for a month without falling back to the terminal. This is calendar time; Phase C work continues during it.

### Phase C — 0.3 beta: safety and maintenance

| ID | Package | Size | Tier | Depends on |
| --- | --- | --- | --- | --- |
| C1 | SQLite store (review history, maintainer history, AUR cache) with migration from `reviews.json`; ADR 0005 | M | Safe | — |
| C2 | Risk signals backend: maintainer changed since last install, age, orphaned, out of date, low votes | M | Safe | C1 |
| C3 | Diff heuristics: new network fetches, `curl \| sh`, `npm`/`bun` installs, base64 or obfuscated blobs; defanged malicious fixtures in `tests/fixtures` | M | Safe | — |
| C4 | Risk panel on the detail and review screens; maintainer-change alert on Updates. Hints only, never a "safe" badge | M | Std | C2, C3 contracts |
| C5 | Cache manager: sizes of the pacman and yay caches; cleaning through the queue (`yay -Sc`, `paccache`) as new planner items | M | Safe, arch | — |
| C6 | Maintenance screen: cache cleaner, orphan cleanup, `yay -Ps` stats, pacman.log history (read-only parser) | M | Std | C5 contract |
| C7 | Foreign package audit: AUR/foreign list with maintainer, age, "no longer in the AUR" | S | Std | — |
| C8 | Tray notifier (`apps/notifier`, StatusNotifierItem): update count, interval, autostart off by default; reuses B1 | M | Std | B1 |
| C9 | Ignore/hold: show IgnorePkg; per-app ignore list passed as arguments, without editing pacman.conf | S | Safe | B1 |

Wave 1: C1 + C3 + C5 + C7. Wave 2: C2 + C6 + C8 + C9. Wave 3: C4.

**Exit (PLAN.md):** a changed commit after review aborts the build in CI; the defanged samples are flagged; tag `0.3.0-beta`.

### Phase D — 0.4 release candidate: polish

| ID | Package | Size | Tier | Notes |
| --- | --- | --- | --- | --- |
| D1 | Settings: app preferences persisted in `~/.config`; "skip review" stays session-only | S | Std | |
| D2 | yay options editor and Lua hook manager with raw-file mode; shows a diff and keeps a backup before writing | M | Safe | The only place the app writes the user's yay config |
| D3 | i18n runtime: locale switch, plurals, date and size formats; catalog-completeness test | M | Std | |
| D4 | Bengali catalog | S | Std | Sonnet drafts; **the owner or a native speaker reviews** before it counts as done |
| D5 | Themes: follow system light/dark and accent colour | S | Std | |
| D6 | Accessibility: axe checks in tests, focus order, keyboard reach for every action, contrast | M | Std | |
| D7 | Playwright end-to-end suite against the built UI with the mock backend; screenshots reused in the docs | M | Std | Chromium is already in the agent container |
| D8 | Performance checks: repo search under 300 ms, start under 1 s; virtualised lists verified at 3,000 rows | S | Std | |
| D9 | User guide and docs site; README screenshots | M | Std | Haiku drafts from the screens and tests |
| D10 | Discover extras, *cut first*: browse by repo and group, "recently updated" from the AUR metadata dump, read-only AUR comments, optional-dependency picker after install | L | Std | Split into four packages if kept |
| D11 | polkit mode for pacman steps, *cut first* | M | Safe, arch | PLAN.md lists it as an option, not a requirement |

All of D1 to D9 are file-disjoint after P0.4 and can run three or four at a time.

**Exit (PLAN.md):** no open P1 bugs; two languages at 100%.

### Phase E — 1.0 release

| ID | Package | Size | Tier | Notes |
| --- | --- | --- | --- | --- |
| E1 | `packaging/aur/` for `yaygui` and `yaygui-bin`, AppStream metainfo, notifier autostart entry | S | Safe | Two-approval rule applies |
| E2 | Release workflow: build on tag, checksums, signatures, GitHub Release, changelog (`git-cliff`), PR that bumps the AUR PKGBUILD | M | Safe | Signing key is the owner's |
| E3 | Whole-repo security pass: `/security-review` on `main`, Tauri capabilities and CSP, `cargo deny`, `pnpm audit` | M | Safe | Findings become their own small packages |
| E4 | Release checklist run against section 2; ADR statuses updated; PLAN.md "next steps" ticked | S | Std | |

## 5. Order and parallelism at a glance

```
P0.1 → P0.2 ─┬→ P0.5 → A6 ──────────────┐
       P0.3 ─┴→ P0.4 → A1,A3,A5 → A2,A7 → A4 → 0.1 alpha
                         B1,B4 → B2,B3,B5 → B6 → 0.2   (month of real use starts)
                         C1,C3,C5,C7 → C2,C6,C8,C9 → C4 → 0.3 beta
                         D1..D9 in waves of 3–4 → 0.4 RC
                         E1,E2 → E3 → E4 → 1.0
```

Phases B and C can overlap once A is merged, because their packages touch different crates and screens. The safety-tier packages are the bottleneck on purpose: each waits for two reviews and the owner.

## 6. What only the owner can do

| When | Decision or action |
| --- | --- |
| Before A7 | **Pick the final name.** It is baked into the app identifier, crate names and data paths; changing it after 0.1 means migrating users' data |
| Before P0.2 | Allow Actions minutes for the Arch workflow, or accept that it only runs if Docker works in the agent container |
| Any time | Say whether the orchestrator may merge standard-tier PRs itself |
| After P0.5 | Contact the yay maintainer about the project and the hook use (PLAN.md), especially if a hook field is missing |
| Phase B exit | Use the app for the weekly upgrade for a month |
| D4 | Review the Bengali translation |
| E1, E2 | AUR account and co-maintainer, release signing key, publishing the packages |
| 1.0 | Announcement |

## 7. Risks specific to building this with agents

| Risk | Mitigation |
| --- | --- |
| Agents write plausible code for yay behaviour nobody has observed | P0.2 comes first; **arch** packages cannot merge without the container suite; fixtures are recorded, never invented |
| A worker quietly weakens a safety rule to make a test pass | Safety tier: two independent reviewers who see the rules and the diff, plus the owner; safety tests live in `crates/core` and are off-limits to other packages' briefs |
| Parallel workers collide | P0.4 file split; waves are file-disjoint; one owner per file per wave |
| Costs creep through retries | Stop rules in 3.4; escalation is a decision by the orchestrator, not automatic |
| The plan drifts from reality | The orchestrator ticks packages here in the same PR that completes them |
| Scope creep | D10 and D11 are cut first; "Later" tier stays out |

## 8. Starting

1. Owner answers the first three rows of section 6.
2. Orchestrator runs P0.1 and commits the brief template.
3. P0.2 and P0.3 start as the first parallel wave.

Progress is tracked by ticking the IDs below in the PR that completes each one.

- [ ] P0.1 · [ ] P0.2 · [ ] P0.3 · [ ] P0.4 · [ ] P0.5
- [ ] A1 · [ ] A2 · [ ] A3 · [ ] A4 · [ ] A5 · [ ] A6 · [ ] A7
- [ ] B1 · [ ] B2 · [ ] B3 · [ ] B4 · [ ] B5 · [ ] B6
- [ ] C1 · [ ] C2 · [ ] C3 · [ ] C4 · [ ] C5 · [ ] C6 · [ ] C7 · [ ] C8 · [ ] C9
- [ ] D1 · [ ] D2 · [ ] D3 · [ ] D4 · [ ] D5 · [ ] D6 · [ ] D7 · [ ] D8 · [ ] D9 · [ ] D10 · [ ] D11
- [ ] E1 · [ ] E2 · [ ] E3 · [ ] E4
