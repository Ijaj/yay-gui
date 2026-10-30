//! Transaction planner: queue -> ordered `yay` argument arrays.

use crate::name::PackageName;
use crate::types::{PackageRef, QueueItem, RemoveMode, ReviewMap, ReviewStatus, Source};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Program every planned command runs.
pub const PROGRAM: &str = "yay";

/// One command to spawn: a program plus an argument array (never a shell string).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedCommand {
    /// Executable name, always `"yay"`.
    pub program: String,
    /// Arguments, passed verbatim.
    pub args: Vec<String>,
    /// Human-readable summary for the UI.
    pub description: String,
}

/// Facts the planner needs from the outside world.
#[derive(Debug, Clone, Default)]
pub struct PlanContext {
    /// GUI review state per AUR package (missing = not reviewed).
    pub reviews: ReviewMap,
    /// True if the sync DBs are not newer than the last full upgrade.
    pub system_up_to_date: bool,
    /// Explicit per-session opt-in to skip review (disables the review check).
    pub skip_review_session_opt_in: bool,
}

/// Why a queue cannot be turned into commands.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlanError {
    /// Repo installs without a full upgrade would be a partial upgrade.
    #[error("installing repo packages ({}) without a full system upgrade is a partial upgrade", join(.0))]
    PartialUpgrade(Vec<PackageName>),
    /// AUR packages lack an approved review.
    #[error("AUR packages not reviewed: {}", join(.0))]
    Unreviewed(Vec<PackageName>),
    /// The same package is queued for install and removal.
    #[error("package {0} is queued for both install and removal")]
    Conflict(PackageName),
    /// Local-only packages cannot be installed.
    #[error("package {0} has no repository or AUR source and cannot be installed")]
    NotInstallable(PackageName),
}

fn join(names: &[PackageName]) -> String {
    names
        .iter()
        .map(PackageName::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}

fn cmd(args: Vec<String>, description: String) -> PlannedCommand {
    PlannedCommand {
        program: PROGRAM.into(),
        args,
        description,
    }
}

fn strs(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| (*s).to_string()).collect()
}

/// Turns a queue into ordered yay commands, enforcing the safety rules.
///
/// Order: install/upgrade, then removals (one command per [`RemoveMode`]),
/// then orphan cleanup. Returns an empty vec for an empty queue. When AUR
/// packages were reviewed in the GUI, `--answerdiff None --answeredit None`
/// is added; with the review opt-in or no AUR installs it is not.
pub fn plan(queue: &[QueueItem], ctx: &PlanContext) -> Result<Vec<PlannedCommand>, PlanError> {
    let mut installs: Vec<&PackageRef> = Vec::new();
    let mut removes: Vec<(&PackageName, RemoveMode)> = Vec::new();
    let (mut upgrade, mut devel, mut orphans) = (false, false, false);
    let mut seen_i = HashSet::new();
    let mut seen_r = HashSet::new();

    for item in queue {
        match item {
            QueueItem::Install(p) => {
                if p.source == Source::Local {
                    return Err(PlanError::NotInstallable(p.name.clone()));
                }
                if seen_i.insert(&p.name) {
                    installs.push(p);
                }
            }
            QueueItem::Remove { name, mode } => {
                if seen_r.insert(name) {
                    removes.push((name, *mode));
                }
            }
            QueueItem::SystemUpgrade { devel: d } => {
                upgrade = true;
                devel |= d;
            }
            QueueItem::CleanOrphans => orphans = true,
        }
    }

    if let Some(p) = installs.iter().find(|p| seen_r.contains(&p.name)) {
        return Err(PlanError::Conflict(p.name.clone()));
    }

    let repo_names: Vec<PackageName> = installs
        .iter()
        .filter(|p| matches!(p.source, Source::Repo(_)))
        .map(|p| p.name.clone())
        .collect();
    if !repo_names.is_empty() && !upgrade && !ctx.system_up_to_date {
        return Err(PlanError::PartialUpgrade(repo_names));
    }

    let aur: Vec<&PackageName> = installs
        .iter()
        .filter(|p| p.source == Source::Aur)
        .map(|p| &p.name)
        .collect();
    if !ctx.skip_review_session_opt_in {
        let bad: Vec<PackageName> = aur
            .iter()
            .filter(|n| !matches!(ctx.reviews.get(**n), Some(ReviewStatus::Approved { .. })))
            .map(|n| (*n).clone())
            .collect();
        if !bad.is_empty() {
            return Err(PlanError::Unreviewed(bad));
        }
    }

    let mut out = Vec::new();

    if upgrade || !installs.is_empty() {
        let mut args = strs(if upgrade { &["-Syu"] } else { &["-S"] });
        if !installs.is_empty() {
            args.push("--needed".into());
        }
        if upgrade && devel {
            args.push("--devel".into());
        }
        if !aur.is_empty() && !ctx.skip_review_session_opt_in {
            args.extend(strs(&["--answerdiff", "None", "--answeredit", "None"]));
        }
        let mut desc = String::new();
        if upgrade {
            desc.push_str(if devel {
                "Full system upgrade (including devel packages)"
            } else {
                "Full system upgrade"
            });
        }
        if !installs.is_empty() {
            let names: Vec<&str> = installs.iter().map(|p| p.name.as_str()).collect();
            if upgrade {
                desc.push_str(" and install ");
            } else {
                desc.push_str("Install ");
            }
            desc.push_str(&names.join(", "));
            args.push("--".into());
            args.extend(names.iter().map(|n| (*n).to_string()));
        }
        out.push(cmd(args, desc));
    }

    for (mode, flag) in [
        (RemoveMode::Plain, "-R"),
        (RemoveMode::WithDeps, "-Rs"),
        (RemoveMode::WithDepsAndConfig, "-Rns"),
    ] {
        let names: Vec<&str> = removes
            .iter()
            .filter(|(_, m)| *m == mode)
            .map(|(n, _)| n.as_str())
            .collect();
        if names.is_empty() {
            continue;
        }
        let mut args = strs(&[flag, "--"]);
        args.extend(names.iter().map(|n| (*n).to_string()));
        out.push(cmd(args, format!("Remove {} ({flag})", names.join(", "))));
    }

    if orphans {
        out.push(cmd(strs(&["-Yc"]), "Remove orphaned packages".into()));
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(s: &str) -> PackageName {
        s.parse().unwrap()
    }
    fn repo(s: &str) -> QueueItem {
        QueueItem::Install(PackageRef {
            name: n(s),
            source: Source::Repo("extra".into()),
        })
    }
    fn aur(s: &str) -> QueueItem {
        QueueItem::Install(PackageRef {
            name: n(s),
            source: Source::Aur,
        })
    }
    fn rm(s: &str, mode: RemoveMode) -> QueueItem {
        QueueItem::Remove { name: n(s), mode }
    }
    fn approved(names: &[&str]) -> PlanContext {
        let mut ctx = PlanContext::default();
        for s in names {
            ctx.reviews.insert(
                n(s),
                ReviewStatus::Approved {
                    commit: "abc".into(),
                },
            );
        }
        ctx
    }
    fn args(c: &PlannedCommand) -> Vec<&str> {
        c.args.iter().map(String::as_str).collect()
    }

    #[test]
    fn empty_queue_no_commands() {
        assert!(plan(&[], &PlanContext::default()).unwrap().is_empty());
    }

    #[test]
    fn partial_upgrade_rejected() {
        let err = plan(&[repo("vim")], &PlanContext::default()).unwrap_err();
        assert_eq!(err, PlanError::PartialUpgrade(vec![n("vim")]));
    }

    #[test]
    fn repo_install_when_up_to_date() {
        let ctx = PlanContext {
            system_up_to_date: true,
            ..Default::default()
        };
        let out = plan(&[repo("vim"), repo("git")], &ctx).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].program, "yay");
        assert_eq!(args(&out[0]), ["-S", "--needed", "--", "vim", "git"]);
    }

    #[test]
    fn upgrade_merged_with_installs() {
        let q = [QueueItem::SystemUpgrade { devel: false }, repo("vim")];
        let out = plan(&q, &PlanContext::default()).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(args(&out[0]), ["-Syu", "--needed", "--", "vim"]);
    }

    #[test]
    fn upgrade_only_and_devel() {
        let out = plan(
            &[QueueItem::SystemUpgrade { devel: false }],
            &PlanContext::default(),
        )
        .unwrap();
        assert_eq!(args(&out[0]), ["-Syu"]);
        let q = [
            QueueItem::SystemUpgrade { devel: false },
            QueueItem::SystemUpgrade { devel: true },
        ];
        let out = plan(&q, &PlanContext::default()).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(args(&out[0]), ["-Syu", "--devel"]);
    }

    #[test]
    fn never_sy_without_u() {
        let ctx = approved(&["foo"]);
        let q = [
            QueueItem::SystemUpgrade { devel: true },
            repo("vim"),
            aur("foo"),
            rm("bar", RemoveMode::WithDeps),
            QueueItem::CleanOrphans,
        ];
        for c in plan(&q, &ctx).unwrap() {
            assert!(!c.args.iter().any(|a| a == "-Sy"));
        }
    }

    #[test]
    fn aur_requires_review() {
        let err = plan(&[aur("foo")], &PlanContext::default()).unwrap_err();
        assert_eq!(err, PlanError::Unreviewed(vec![n("foo")]));
        let mut ctx = PlanContext::default();
        ctx.reviews
            .insert(n("foo"), ReviewStatus::ChangedSinceReview);
        assert_eq!(
            plan(&[aur("foo")], &ctx).unwrap_err(),
            PlanError::Unreviewed(vec![n("foo")])
        );
        ctx.reviews.insert(n("foo"), ReviewStatus::NotReviewed);
        assert!(plan(&[aur("foo")], &ctx).is_err());
    }

    #[test]
    fn partially_reviewed_lists_only_missing() {
        let err = plan(&[aur("a"), aur("b")], &approved(&["a"])).unwrap_err();
        assert_eq!(err, PlanError::Unreviewed(vec![n("b")]));
    }

    #[test]
    fn reviewed_aur_adds_answer_flags() {
        let out = plan(&[aur("foo")], &approved(&["foo"])).unwrap();
        assert_eq!(
            args(&out[0]),
            [
                "-S",
                "--needed",
                "--answerdiff",
                "None",
                "--answeredit",
                "None",
                "--",
                "foo"
            ]
        );
    }

    #[test]
    fn skip_review_opt_in() {
        let ctx = PlanContext {
            skip_review_session_opt_in: true,
            ..Default::default()
        };
        let out = plan(&[aur("foo")], &ctx).unwrap();
        assert_eq!(args(&out[0]), ["-S", "--needed", "--", "foo"]);
    }

    #[test]
    fn aur_does_not_need_upgrade() {
        // AUR-only installs are not subject to the partial-upgrade rule.
        assert!(plan(&[aur("foo")], &approved(&["foo"])).is_ok());
    }

    #[test]
    fn removals_separate_and_ordered() {
        let q = [
            QueueItem::CleanOrphans,
            rm("a", RemoveMode::WithDepsAndConfig),
            rm("b", RemoveMode::WithDepsAndConfig),
            rm("c", RemoveMode::Plain),
            QueueItem::SystemUpgrade { devel: false },
        ];
        let out = plan(&q, &PlanContext::default()).unwrap();
        let all: Vec<Vec<&str>> = out.iter().map(args).collect();
        assert_eq!(
            all,
            vec![
                vec!["-Syu"],
                vec!["-R", "--", "c"],
                vec!["-Rns", "--", "a", "b"],
                vec!["-Yc"]
            ]
        );
    }

    #[test]
    fn conflict_rejected() {
        let ctx = PlanContext {
            system_up_to_date: true,
            ..Default::default()
        };
        let err = plan(&[repo("vim"), rm("vim", RemoveMode::Plain)], &ctx).unwrap_err();
        assert_eq!(err, PlanError::Conflict(n("vim")));
    }

    #[test]
    fn local_not_installable() {
        let q = [QueueItem::Install(PackageRef {
            name: n("x"),
            source: Source::Local,
        })];
        assert_eq!(
            plan(&q, &PlanContext::default()).unwrap_err(),
            PlanError::NotInstallable(n("x"))
        );
    }

    #[test]
    fn duplicates_collapsed() {
        let ctx = PlanContext {
            system_up_to_date: true,
            ..Default::default()
        };
        let out = plan(&[repo("vim"), repo("vim")], &ctx).unwrap();
        assert_eq!(args(&out[0]), ["-S", "--needed", "--", "vim"]);
    }

    #[test]
    fn separator_precedes_all_names() {
        let ctx = PlanContext {
            system_up_to_date: true,
            ..Default::default()
        };
        let q = [repo("vim"), rm("old", RemoveMode::Plain)];
        for c in plan(&q, &ctx).unwrap() {
            let i = c.args.iter().position(|a| a == "--").unwrap();
            assert!(c.args[..i].iter().all(|a| a.starts_with('-')));
            assert!(c.args[i + 1..].iter().all(|a| !a.starts_with('-')));
        }
    }
}
