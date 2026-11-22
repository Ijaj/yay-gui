//! The scenarios and the transaction driver.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use aurdeck_alpm_reader::{FsConfig, FsDb, InstalledFilter, PackageDb, repos_from_pacman_conf};
use aurdeck_core::{
    PackageName, PackageRef, PlanContext, PlannedCommand, QueueItem, RemoveMode, ReviewMap,
    ReviewStatus, Source, TransactionState, plan,
};
use aurdeck_review::{AurGit, Fetched, ReviewStore, review_status};
use aurdeck_yay_runner::{
    ApprovedBase, Overlay, PromptEvent, TxConfig, TxEvent, start_transaction,
};
use zeroize::Zeroizing;

use crate::prompts;
use crate::screen::{Decision, Screen};
use crate::{Cli, Which};

/// Small, stable repo package (extra): a few dozen KiB, depends only on glibc.
pub const REPO_PACKAGE: &str = "tree";
/// Small AUR-only package: `arch=('any')`, depends on pacman-contrib and fzf (repo packages), source
/// is one GitHub release tarball, builds in a second or two.
pub const AUR_PACKAGE: &str = "downgrade";

/// A scenario ends if it takes longer than this without finishing.
const TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// After a cancel, how long the driver waits for the transaction to end
/// before it gives up and exits the process.
const CANCEL_GRACE: Duration = Duration::from_secs(15);

/// Written by the Dockerfile: readable by `dev` only. Not in the environment,
/// so yay, makepkg and PKGBUILDs do not inherit it.
const PASSWORD_FILE: &str = "/etc/aurdeck-harness-sudo-password";

type Res<T = ()> = Result<T, String>;

pub fn run(cli: &Cli) -> Res {
    let pw = read_password()?;
    match cli.which {
        Which::RepoInstall => repo_install(&pw),
        Which::AurInstall => aur_install(&pw, cli.no_overlay),
        Which::PinCommitMismatch => pin_commit_mismatch(&pw),
        Which::PinUnapprovedBase => pin_unapproved_base(&pw),
        Which::PinUnreviewedUpgrade => pin_unreviewed_upgrade(&pw),
        Which::Remove => remove(&pw),
        Which::All => {
            repo_install(&pw)?;
            pin_commit_mismatch(&pw)?;
            pin_unapproved_base(&pw)?;
            pin_unreviewed_upgrade(&pw)?;
            aur_install(&pw, cli.no_overlay)?;
            remove(&pw)
        }
    }
}

fn read_password() -> Res<Zeroizing<String>> {
    let raw = Zeroizing::new(
        std::fs::read_to_string(PASSWORD_FILE)
            .map_err(|e| format!("cannot read {PASSWORD_FILE}: {e}"))?,
    );
    Ok(Zeroizing::new(raw.trim_end_matches('\n').to_owned()))
}

/// Foreign (AUR or local) packages and their versions.
fn foreign() -> Res<BTreeMap<String, String>> {
    Ok(open_db()?
        .installed(InstalledFilter::Foreign)
        .into_iter()
        .map(|p| (p.name, p.version))
        .collect())
}

/// A scenario that is not about the AUR must not build or change any AUR package.
fn expect_foreign_unchanged(before: &BTreeMap<String, String>) -> Res {
    let after = foreign()?;
    if &after == before {
        println!("post-condition ok: no AUR or foreign package was built or changed");
        Ok(())
    } else {
        Err(format!(
            "an AUR or foreign package changed during a repo scenario: before {before:?}, after {after:?}"
        ))
    }
}

/// HEAD of the clone yay builds from (`~/.cache/yay/<pkgbase>`).
fn built_commit(pkgbase: &str) -> Res<String> {
    let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
    let dir = PathBuf::from(home).join(".cache/yay").join(pkgbase);
    let out = std::process::Command::new("git")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("LC_ALL", "C")
        .arg("-C")
        .arg(&dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|e| format!("cannot run the commit lookup: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cannot read the commit yay built in {}: {}",
            dir.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn banner(s: &str) {
    println!("\n=== scenario: {s} ===");
}

fn name(s: &str) -> Res<PackageName> {
    s.parse().map_err(|e| format!("bad package name {s}: {e}"))
}

fn open_db() -> Res<FsDb> {
    let repos = std::fs::read_to_string("/etc/pacman.conf")
        .map(|t| repos_from_pacman_conf(&t))
        .map_err(|e| format!("cannot read /etc/pacman.conf: {e}"))?;
    FsDb::open(&FsConfig {
        repos,
        ..FsConfig::default()
    })
    .map_err(|e| format!("cannot read the pacman databases: {e}"))
}

/// Version of the installed package, read from the real local database.
fn installed_version(pkg: &str) -> Res<Option<String>> {
    Ok(open_db()?.local_package(pkg).map(|p| p.version))
}

fn expect_installed(pkg: &str) -> Res {
    match installed_version(pkg)? {
        Some(v) => {
            println!("post-condition ok: {pkg} {v} is installed");
            Ok(())
        }
        None => Err(format!("post-condition failed: {pkg} is not installed")),
    }
}

fn expect_absent(pkg: &str, why: &str) -> Res {
    match installed_version(pkg)? {
        None => {
            println!("{why} ok: {pkg} is not installed");
            Ok(())
        }
        Some(v) => Err(format!("{why} failed: {pkg} {v} is installed")),
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

fn repo_install(pw: &str) -> Res {
    banner("repo-install");
    expect_absent(REPO_PACKAGE, "precondition")?;
    let foreign_before = foreign()?;
    let db = open_db()?;
    let sync = db.sync_package(REPO_PACKAGE).ok_or_else(|| {
        format!("{REPO_PACKAGE} is not in the sync databases (run pacman -Sy at image build)")
    })?;
    // The planner refuses repo installs without a full upgrade, so the queue
    // carries one, exactly as the app does when the system is out of date.
    let queue = vec![
        QueueItem::SystemUpgrade { devel: false },
        QueueItem::Install(PackageRef {
            name: name(REPO_PACKAGE)?,
            source: Source::Repo(sync.repo),
        }),
    ];
    let commands = plan(&queue, &PlanContext::default()).map_err(|e| format!("plan: {e}"))?;
    drive(commands, Vec::new(), pw)?;
    expect_installed(REPO_PACKAGE)?;
    expect_foreign_unchanged(&foreign_before)
}

/// An AUR package base fetched like the app's review screen does: a bare clone
/// in a temp dir, kept alive with this value.
struct Fetch {
    _tmp: tempfile::TempDir,
    git_dir: PathBuf,
    fetched: Fetched,
    store: ReviewStore,
}

impl Fetch {
    fn pkgbuild(&self) -> Res<String> {
        pkgbuild_of(&self.fetched)
    }

    /// The commit before HEAD of the package's history.
    fn older_commit(&self) -> Res<String> {
        let out = git_out(
            &self.git_dir,
            &[
                "rev-list",
                "--first-parent",
                "-n",
                "2",
                &self.fetched.commit,
            ],
        )?;
        let older = out
            .lines()
            .nth(1)
            .ok_or_else(|| format!("{} has no older commit", self.fetched.pkgbase))?
            .trim()
            .to_owned();
        if !aurdeck_review::is_commit_hash(&older) {
            return Err(format!("bad older commit {older:?}"));
        }
        Ok(older)
    }

    /// PKGBUILD text at `commit`.
    fn pkgbuild_at(&self, commit: &str) -> Res<String> {
        git_out(&self.git_dir, &["show", &format!("{commit}:PKGBUILD")])
    }
}

fn pkgbuild_of(fetched: &Fetched) -> Res<String> {
    fetched
        .files
        .iter()
        .find(|f| f.path == "PKGBUILD")
        .and_then(|f| f.content.clone())
        .ok_or_else(|| format!("{} has no readable PKGBUILD", fetched.pkgbase))
}

/// Runs git with the user's and the system's config ignored; stdout as text.
fn git_out(git_dir: &Path, args: &[&str]) -> Res<String> {
    let out = std::process::Command::new("git")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("LC_ALL", "C")
        .arg("--git-dir")
        .arg(git_dir)
        .args(args)
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Same steps as the app's review screen: fetch the PKGBUILD repository and
/// check it is its own base with a PKGBUILD. Nothing is approved yet.
fn fetch_aur(pkgname: &str) -> Res<Fetch> {
    let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let store = ReviewStore::open(tmp.path().join("reviews.json")).map_err(|e| e.to_string())?;
    let git = AurGit::new(tmp.path().join("aur"));
    let pkg = name(pkgname)?;
    let fetched = git
        .fetch(&pkg)
        .map_err(|e| format!("fetch {pkgname}: {e}"))?;
    if fetched.pkgbase != pkgname {
        return Err(format!(
            "{pkgname} has pkgbase {}; the test package must be its own base",
            fetched.pkgbase
        ));
    }
    pkgbuild_of(&fetched)?;
    println!(
        "fetched {} at {} ({} files)",
        fetched.pkgbase,
        fetched.commit,
        fetched.files.len()
    );
    let git_dir = tmp.path().join("aur").join(format!("{pkgname}.git"));
    Ok(Fetch {
        _tmp: tmp,
        git_dir,
        fetched,
        store,
    })
}

/// The test AUR package must be buildable from the AUR: absent, not in a sync repo.
fn aur_precondition(pkg: &str) -> Res {
    expect_absent(pkg, "precondition")?;
    // yay installs a repo package of the same name instead of building from the AUR.
    if open_db()?.sync_package(pkg).is_some() {
        return Err(format!(
            "{pkg} is now in a sync repo, so this scenario would not build from the AUR; pick another test package"
        ));
    }
    Ok(())
}

fn user_config() -> Res<PathBuf> {
    std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join(".config"))
        .ok_or_else(|| "HOME is not set".to_owned())
}

/// A plan context in which the planner accepts exactly these AUR packages.
fn ctx_with(reviews: Vec<(&str, ReviewStatus)>) -> Res<PlanContext> {
    let mut map = ReviewMap::new();
    for (n, s) in reviews {
        map.insert(name(n)?, s);
    }
    Ok(PlanContext {
        reviews: map,
        // Only AUR packages are queued (or a full upgrade is), so the
        // partial-upgrade rule does not apply.
        system_up_to_date: true,
        skip_review_session_opt_in: false,
    })
}

fn install_aur(pkgname: &str) -> Res<QueueItem> {
    Ok(QueueItem::Install(PackageRef {
        name: name(pkgname)?,
        source: Source::Aur,
    }))
}

fn approved_at(f: &Fetch, commit: &str, pkgbuild: String) -> BTreeMap<String, ApprovedBase> {
    BTreeMap::from([(
        f.fetched.pkgbase.clone(),
        ApprovedBase {
            commit: commit.to_owned(),
            pkgbuild: Some(pkgbuild),
        },
    )])
}

fn aur_install(pw: &str, no_overlay: bool) -> Res {
    banner(if no_overlay {
        "aur-install (--no-overlay: diagnostic, review pinning is OFF)"
    } else {
        "aur-install"
    });
    aur_precondition(AUR_PACKAGE)?;
    let foreign_before = foreign()?;

    // Show it (here: print the file list), approve exactly that commit.
    let mut f = fetch_aur(AUR_PACKAGE)?;
    let fetched = f.fetched.clone();
    f.store
        .approve(&fetched.pkgbase, &fetched.commit, None, now())
        .map_err(|e| e.to_string())?;
    let status = review_status(
        f.store.latest(&fetched.pkgbase),
        Some(&fetched.commit),
        None,
    );
    let ctx = ctx_with(vec![(AUR_PACKAGE, status)])?;
    let commands = plan(&[install_aur(AUR_PACKAGE)?], &ctx).map_err(|e| format!("plan: {e}"))?;

    let approved = approved_at(&f, &fetched.commit, f.pkgbuild()?);
    let overlay = if no_overlay {
        None
    } else {
        Some(
            Overlay::create_pinned(&user_config()?, &approved)
                .map_err(|e| format!("overlay: {e}"))?,
        )
    };
    let env = overlay.as_ref().map(Overlay::env).unwrap_or_default();
    let result = drive(commands, env, pw);
    drop(overlay);
    result?;
    expect_installed(AUR_PACKAGE)?;

    // Only the reviewed package may have been added (its dependencies are repo packages).
    let after = foreign()?;
    let changed: Vec<&String> = after
        .keys()
        .filter(|k| foreign_before.get(*k) != after.get(*k))
        .collect();
    if changed.len() != 1 || changed[0] != AUR_PACKAGE || foreign_before.len() + 1 != after.len() {
        return Err(format!(
            "expected exactly {AUR_PACKAGE} to be added; foreign packages before {foreign_before:?}, after {after:?}"
        ));
    }
    // yay must have built the commit that was reviewed and approved.
    let built = built_commit(&fetched.pkgbase)?;
    if built != fetched.commit {
        return Err(format!(
            "yay built {built} but the approved commit is {}",
            fetched.commit
        ));
    }
    println!("post-condition ok: yay built the approved commit {built}");
    Ok(())
}

/// The transaction must have failed, and yay's output must carry the overlay's
/// `aurdeck:` message naming `pkgbase` (and `needle` when given); nothing was built.
fn expect_pin_abort(out: &Outcome, pkgbase: &str, needle: Option<&str>) -> Res {
    if out.state == TransactionState::Succeeded {
        return Err("the transaction succeeded but the pin had to abort it".into());
    }
    let text = &out.text;
    let line = text
        .lines()
        .find(|l| l.contains("aurdeck:") && l.contains(pkgbase));
    let Some(line) = line else {
        return Err(format!(
            "yay's output has no `aurdeck:` line naming {pkgbase}; the abort did not come from the overlay"
        ));
    };
    if let Some(n) = needle
        && !line.contains(n)
    {
        return Err(format!("abort line lacks {n:?}: {line:?}"));
    }
    if text.contains("Making package") {
        return Err("yay started building a package before the pin aborted".into());
    }
    println!("post-condition ok: yay aborted with {:?}", line.trim());
    Ok(())
}

fn pin_commit_mismatch(pw: &str) -> Res {
    banner("pin-commit-mismatch");
    aur_precondition(AUR_PACKAGE)?;
    let mut f = fetch_aur(AUR_PACKAGE)?;
    let old = f.older_commit()?;
    let old_pkgbuild = f.pkgbuild_at(&old)?;
    println!(
        "approving the older commit {old}; yay will fetch {}",
        f.fetched.commit
    );
    f.store
        .approve(&f.fetched.pkgbase, &old, None, now())
        .map_err(|e| e.to_string())?;
    let status = review_status(f.store.latest(&f.fetched.pkgbase), Some(&old), None);
    let ctx = ctx_with(vec![(AUR_PACKAGE, status)])?;
    let commands = plan(&[install_aur(AUR_PACKAGE)?], &ctx).map_err(|e| format!("plan: {e}"))?;
    let overlay = Overlay::create_pinned(&user_config()?, &approved_at(&f, &old, old_pkgbuild))
        .map_err(|e| format!("overlay: {e}"))?;
    let out = run_tx(commands, overlay.env(), pw, prompts::EXCLUDE_AUR_UPGRADES)?;
    drop(overlay);
    expect_pin_abort(&out, AUR_PACKAGE, Some("changed since review"))?;
    expect_absent(AUR_PACKAGE, "post-condition")
}

fn pin_unapproved_base(pw: &str) -> Res {
    banner("pin-unapproved-base");
    aur_precondition(AUR_PACKAGE)?;
    let f = fetch_aur(AUR_PACKAGE)?;
    // Forged: the planner is told the package is approved, the overlay is not.
    let forged = ReviewStatus::Approved {
        commit: f.fetched.commit.clone(),
    };
    let ctx = ctx_with(vec![(AUR_PACKAGE, forged)])?;
    let commands = plan(&[install_aur(AUR_PACKAGE)?], &ctx).map_err(|e| format!("plan: {e}"))?;
    let overlay = Overlay::create_pinned(&user_config()?, &BTreeMap::new())
        .map_err(|e| format!("overlay: {e}"))?;
    let out = run_tx(commands, overlay.env(), pw, prompts::EXCLUDE_AUR_UPGRADES)?;
    drop(overlay);
    expect_pin_abort(&out, AUR_PACKAGE, Some("was not reviewed"))?;
    expect_absent(AUR_PACKAGE, "post-condition")
}

/// A second small AUR package: `arch=(any)`, one local source file, depends only on
/// `pacman-contrib` (already pulled in by `downgrade`).
pub const AUR_PACKAGE_2: &str = "pacman-cleanup-hook";

/// Installs the older commit of `f` with makepkg as `dev` and pacman -U (setup, not under test).
fn install_old_build(f: &Fetch, old: &str, pw: &str) -> Res {
    use std::io::Write as _;
    let work = tempfile::tempdir().map_err(|e| e.to_string())?;
    let src = work.path().join("src");
    let run = |cmd: &mut std::process::Command, what: &str| -> Res {
        let out = cmd.output().map_err(|e| format!("{what}: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(format!(
                "{what} failed: {}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ))
        }
    };
    let env = |c: &mut std::process::Command| {
        c.env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("LC_ALL", "C");
    };
    let mut clone = std::process::Command::new("git");
    env(&mut clone);
    clone
        .args(["clone", "--quiet", "--"])
        .arg(&f.git_dir)
        .arg(&src);
    run(&mut clone, "git clone")?;
    let mut checkout = std::process::Command::new("git");
    env(&mut checkout);
    checkout
        .arg("-C")
        .arg(&src)
        .args(["checkout", "--quiet", old]);
    run(&mut checkout, "git checkout")?;
    let mut make = std::process::Command::new("makepkg");
    make.current_dir(&src)
        .env("PKGDEST", work.path())
        .args(["--noconfirm", "--nodeps"]);
    run(&mut make, "makepkg")?;
    let pkg = std::fs::read_dir(work.path())
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().ends_with(".pkg.tar.zst"))
        })
        .ok_or("makepkg produced no package")?;
    let mut sudo = std::process::Command::new("sudo")
        .args([
            "-S",
            "-p",
            "",
            "pacman",
            "-U",
            "--noconfirm",
            "--needed",
            "--",
        ])
        .arg(&pkg)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("sudo: {e}"))?;
    {
        let mut stdin = sudo.stdin.take().ok_or("no stdin")?;
        stdin
            .write_all(format!("{pw}\n").as_bytes())
            .map_err(|e| e.to_string())?;
    }
    let out = sudo.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "pacman -U of the old build failed: {}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

fn pin_unreviewed_upgrade(pw: &str) -> Res {
    banner("pin-unreviewed-upgrade");
    aur_precondition(AUR_PACKAGE)?;
    aur_precondition(AUR_PACKAGE_2)?;
    let old_pkg = fetch_aur(AUR_PACKAGE)?;
    let new_pkg = {
        let mut f = fetch_aur(AUR_PACKAGE_2)?;
        f.store
            .approve(&f.fetched.pkgbase, &f.fetched.commit.clone(), None, now())
            .map_err(|e| e.to_string())?;
        f
    };
    let old = old_pkg.older_commit()?;
    println!("setup: installing {AUR_PACKAGE} at the older commit {old} (not under test)");
    install_old_build(&old_pkg, &old, pw)?;
    let before = installed_version(AUR_PACKAGE)?.ok_or("setup: the old build is not installed")?;
    println!(
        "setup: {AUR_PACKAGE} {before} is installed; the AUR has {}",
        old_pkg.fetched.commit
    );

    let result = pin_unreviewed_upgrade_run(&new_pkg, &old_pkg, &before, pw);
    // Cleanup, whatever happened: remove the setup package through the planner.
    println!("cleanup: removing {AUR_PACKAGE} (not under test)");
    let cleanup = name(AUR_PACKAGE).and_then(|n| {
        let q = [QueueItem::Remove {
            name: n,
            mode: RemoveMode::WithDeps,
        }];
        let cmds = plan(&q, &PlanContext::default()).map_err(|e| format!("plan: {e}"))?;
        drive(cmds, Vec::new(), pw)
    });
    result?;
    cleanup?;
    expect_absent(AUR_PACKAGE, "cleanup")
}

fn pin_unreviewed_upgrade_run(new_pkg: &Fetch, old_pkg: &Fetch, before: &str, pw: &str) -> Res {
    let status = review_status(
        new_pkg.store.latest(&new_pkg.fetched.pkgbase),
        Some(&new_pkg.fetched.commit),
        None,
    );
    let ctx = ctx_with(vec![(AUR_PACKAGE_2, status)])?;
    let queue = [
        QueueItem::SystemUpgrade { devel: false },
        install_aur(AUR_PACKAGE_2)?,
    ];
    let commands = plan(&queue, &ctx).map_err(|e| format!("plan: {e}"))?;
    // Only the new package is approved; the pending AUR upgrade of AUR_PACKAGE is not.
    let approved = approved_at(new_pkg, &new_pkg.fetched.commit, new_pkg.pkgbuild()?);
    let overlay =
        Overlay::create_pinned(&user_config()?, &approved).map_err(|e| format!("overlay: {e}"))?;
    // Nothing is excluded, so the AUR upgrade is really attempted.
    let out = run_tx(commands, overlay.env(), pw, prompts::EXCLUDE_NOTHING)?;
    drop(overlay);
    expect_pin_abort(&out, &old_pkg.fetched.pkgbase, Some("was not reviewed"))?;
    if installed_version(AUR_PACKAGE)?.as_deref() != Some(before) {
        return Err(format!(
            "{AUR_PACKAGE} changed from {before} to {:?}",
            installed_version(AUR_PACKAGE)?
        ));
    }
    println!("post-condition ok: {AUR_PACKAGE} is still {before}");
    expect_absent(AUR_PACKAGE_2, "post-condition")
}

fn remove(pw: &str) -> Res {
    banner("remove");
    if installed_version(REPO_PACKAGE)?.is_none() {
        println!(
            "{REPO_PACKAGE} is not installed yet; installing it first (setup, not under test)"
        );
        repo_install(pw)?;
        banner("remove (continued)");
    }
    let queue = vec![QueueItem::Remove {
        name: name(REPO_PACKAGE)?,
        mode: RemoveMode::WithDeps,
    }];
    let commands = plan(&queue, &PlanContext::default()).map_err(|e| format!("plan: {e}"))?;
    let foreign_before = foreign()?;
    drive(commands, Vec::new(), pw)?;
    expect_absent(REPO_PACKAGE, "post-condition")?;
    expect_foreign_unchanged(&foreign_before)
}

/// How a transaction ended, and everything it printed.
struct Outcome {
    state: TransactionState,
    /// Terminal output with escape sequences removed.
    text: String,
}

/// Runs the commands with `start_transaction`, printing every event and
/// answering prompts from the explicit table. Succeeds only when the
/// transaction ends in `Succeeded`.
fn drive(commands: Vec<PlannedCommand>, env: Vec<(String, String)>, password: &str) -> Res {
    let out = run_tx(commands, env, password, prompts::EXCLUDE_AUR_UPGRADES)?;
    match out.state {
        TransactionState::Succeeded => Ok(()),
        other => Err(format!("transaction ended in {other:?}")),
    }
}

/// Like [`drive`] but returns how the transaction ended. A harness failure
/// (unrecognised prompt, stall, timeout) is still an `Err`. `exclude` is the
/// answer to yay's `Packages to exclude` menu.
fn run_tx(
    commands: Vec<PlannedCommand>,
    env: Vec<(String, String)>,
    password: &str,
    exclude: &'static str,
) -> Res<Outcome> {
    let (tx, rx) = mpsc::channel();
    let config = TxConfig {
        env,
        ..TxConfig::default()
    };
    let handle = start_transaction(commands, config, move |ev| {
        let _ = tx.send(ev);
    });
    let deadline = Instant::now() + TIMEOUT;
    let mut stdout = std::io::stdout();
    let mut failure: Option<String> = None;
    let mut cancelled_at: Option<Instant> = None;
    let mut final_state: Option<TransactionState> = None;
    let mut screen = Screen::default();
    let mut last_output = Instant::now();
    let mut raw: Vec<u8> = Vec::new();
    let mut current_args: Vec<String> = Vec::new();

    loop {
        if failure.is_none() && Instant::now() >= deadline {
            failure = Some(format!("timed out after {} s", TIMEOUT.as_secs()));
        }
        if failure.is_some() && cancelled_at.is_none() {
            handle.cancel();
            cancelled_at = Some(Instant::now());
        }
        if cancelled_at.is_some_and(|t| t.elapsed() > CANCEL_GRACE) {
            eprintln!(
                "\nHARNESS FAILED: {}; the transaction did not stop after a cancel, exiting",
                failure.as_deref().unwrap_or("cancelled")
            );
            std::process::exit(1);
        }
        let ev = match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(ev) => ev,
            Err(RecvTimeoutError::Timeout) => {
                if failure.is_none() {
                    match screen.poll(last_output.elapsed()) {
                        Decision::Nothing => {}
                        Decision::Fail(why) => failure = Some(why),
                    }
                }
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => break,
        };
        match ev {
            TxEvent::Output(bytes) => {
                last_output = Instant::now();
                screen.output(&bytes);
                raw.extend_from_slice(&bytes);
                let _ = stdout.write_all(String::from_utf8_lossy(&bytes).as_bytes());
                let _ = stdout.flush();
            }
            TxEvent::State(s) => {
                println!("\n[state] {s:?}");
                if s.is_terminal() {
                    final_state = Some(s);
                    break;
                }
            }
            TxEvent::CommandStarted {
                index,
                total,
                description,
                argv,
            } => {
                screen.command_started();
                last_output = Instant::now();
                println!("\n[command {}/{total}] {description}: {argv:?}", index + 1);
                current_args = argv;
            }
            TxEvent::CommandFinished { index, status } => {
                println!("\n[command {} finished] {status:?}", index + 1);
            }
            TxEvent::Prompt(p) => {
                println!("\n[prompt] {p:?}");
                screen.prompt_reported();
                if failure.is_none()
                    && let Err(e) = answer(&handle, &p, password, &current_args, exclude)
                {
                    failure = Some(e);
                }
            }
        }
    }
    handle.wait();

    if let Some(f) = failure {
        return Err(f);
    }
    match final_state {
        Some(state) => Ok(Outcome {
            state,
            text: prompts::strip_ansi(&raw),
        }),
        None => Err("transaction ended without a final state".into()),
    }
}

fn answer(
    handle: &aurdeck_yay_runner::TxHandle,
    p: &PromptEvent,
    password: &str,
    args: &[String],
    exclude: &'static str,
) -> Result<(), String> {
    use aurdeck_yay_runner::ErrorKind;
    use prompts::MenuAction;
    match p {
        PromptEvent::SudoPassword { user } => {
            println!("[answer] sudo password for {user}");
            handle
                .answer_password(Zeroizing::new(password.to_owned()))
                .map_err(|e| format!("could not send the password: {e}"))
        }
        PromptEvent::YesNo { question, .. } => match prompts::yes_no(question) {
            Some(yes) => {
                println!("[answer] {}", if yes { "yes" } else { "no" });
                handle
                    .answer_yes_no(yes)
                    .map_err(|e| format!("could not answer: {e}"))
            }
            None => Err(format!("unrecognised yes/no prompt: {question:?}")),
        },
        PromptEvent::Menu { text } => {
            let kind = p.menu_kind().ok_or("menu without a kind")?;
            match prompts::menu_action(kind, args, exclude) {
                MenuAction::Answer(line) => {
                    println!("[answer] {kind:?}: {line:?}");
                    handle
                        .write_input(format!("{line}\n").as_bytes())
                        .map_err(|e| format!("could not answer: {e}"))
                }
                MenuAction::Ignore => {
                    println!("[answer] none: yay answers the {kind:?} menu itself (plan flag)");
                    Ok(())
                }
                MenuAction::Unknown => Err(format!("unrecognised menu prompt: {text:?}")),
            }
        }
        PromptEvent::Error { kind, line } => {
            // Errors are classified for the log; the transaction state decides
            // pass or fail, except a wrong password, which would just repeat.
            if *kind == ErrorKind::AuthFailed {
                Err(format!("sudo rejected the password: {line}"))
            } else {
                Ok(())
            }
        }
    }
}
