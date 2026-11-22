//! The three scenarios and the transaction driver.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use aurdeck_alpm_reader::{FsConfig, FsDb, InstalledFilter, PackageDb, repos_from_pacman_conf};
use aurdeck_core::{
    PackageName, PackageRef, PlanContext, PlannedCommand, QueueItem, RemoveMode, ReviewMap, Source,
    TransactionState, plan,
};
use aurdeck_review::{AurGit, ReviewStore, review_status};
use aurdeck_yay_runner::{Overlay, PromptEvent, TxConfig, TxEvent, start_transaction};
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
        Which::Remove => remove(&pw),
        Which::All => {
            repo_install(&pw)?;
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

fn aur_install(pw: &str, no_overlay: bool) -> Res {
    banner(if no_overlay {
        "aur-install (--no-overlay: diagnostic, review pinning is OFF)"
    } else {
        "aur-install"
    });
    expect_absent(AUR_PACKAGE, "precondition")?;
    // yay installs a repo package of the same name instead of building from the AUR.
    if open_db()?.sync_package(AUR_PACKAGE).is_some() {
        return Err(format!(
            "{AUR_PACKAGE} is now in a sync repo, so this scenario would not build from the AUR; pick another test package"
        ));
    }

    let foreign_before = foreign()?;

    // Same steps as the app's review screen: fetch the PKGBUILD repository,
    // show it (here: print the file list), approve exactly that commit.
    let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let mut store =
        ReviewStore::open(tmp.path().join("reviews.json")).map_err(|e| e.to_string())?;
    let git = AurGit::new(tmp.path().join("aur"));
    let pkg = name(AUR_PACKAGE)?;
    let fetched = git
        .fetch(&pkg)
        .map_err(|e| format!("fetch {AUR_PACKAGE}: {e}"))?;
    if fetched.pkgbase != AUR_PACKAGE {
        return Err(format!(
            "{AUR_PACKAGE} has pkgbase {}; the test package must be its own base",
            fetched.pkgbase
        ));
    }
    if !fetched.files.iter().any(|f| f.path == "PKGBUILD") {
        return Err("fetched repository has no PKGBUILD".into());
    }
    println!(
        "fetched {} at {} ({} files)",
        fetched.pkgbase,
        fetched.commit,
        fetched.files.len()
    );
    store
        .approve(&fetched.pkgbase, &fetched.commit, None, now())
        .map_err(|e| e.to_string())?;

    let status = review_status(store.latest(&fetched.pkgbase), Some(&fetched.commit), None);
    let mut reviews = ReviewMap::new();
    reviews.insert(pkg.clone(), status);
    let ctx = PlanContext {
        reviews,
        // Only an AUR package is queued, so the partial-upgrade rule does not
        // apply and the full upgrade ran in repo-install / at image build.
        system_up_to_date: true,
        skip_review_session_opt_in: false,
    };
    let queue = vec![QueueItem::Install(PackageRef {
        name: pkg,
        source: Source::Aur,
    })];
    let commands = plan(&queue, &ctx).map_err(|e| format!("plan: {e}"))?;

    let approved = BTreeMap::from([(fetched.pkgbase.clone(), fetched.commit.clone())]);
    let user_config = std::env::var_os("HOME")
        .map(|h| std::path::PathBuf::from(h).join(".config"))
        .ok_or("HOME is not set")?;
    let overlay = if no_overlay {
        None
    } else {
        Some(Overlay::create(&user_config, &approved).map_err(|e| format!("overlay: {e}"))?)
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

/// Runs the commands with `start_transaction`, printing every event and
/// answering prompts from the explicit table. Succeeds only when the
/// transaction ends in `Succeeded`.
fn drive(commands: Vec<PlannedCommand>, env: Vec<(String, String)>, password: &str) -> Res {
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
                        Decision::Answer(line) => {
                            println!(
                                "\n[answer] {line:?} (exclude menu the detector does not report yet)"
                            );
                            if let Err(e) = handle.write_input(format!("{line}\n").as_bytes()) {
                                failure = Some(format!("could not answer: {e}"));
                            }
                        }
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
            }
            TxEvent::CommandFinished { index, status } => {
                println!("\n[command {} finished] {status:?}", index + 1);
            }
            TxEvent::Prompt(p) => {
                println!("\n[prompt] {p:?}");
                screen.prompt_reported();
                if failure.is_none()
                    && let Err(e) = answer(&handle, &p, password)
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
        Some(TransactionState::Succeeded) => Ok(()),
        Some(other) => Err(format!("transaction ended in {other:?}")),
        None => Err("transaction ended without a final state".into()),
    }
}

fn answer(
    handle: &aurdeck_yay_runner::TxHandle,
    p: &PromptEvent,
    password: &str,
) -> Result<(), String> {
    use aurdeck_yay_runner::ErrorKind;
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
        PromptEvent::Menu { text } => match prompts::menu(text) {
            Some(line) => {
                println!("[answer] {line:?}");
                handle
                    .write_input(format!("{line}\n").as_bytes())
                    .map_err(|e| format!("could not answer: {e}"))
            }
            None => Err(format!("unrecognised menu prompt: {text:?}")),
        },
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
