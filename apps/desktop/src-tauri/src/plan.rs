//! Queue DTOs and the pure part of transaction planning.
//!
//! The frontend sends its queue; the backend re-validates every name, checks
//! that "repo" packages really are in the sync databases, works out which AUR
//! packages (including AUR dependencies) need a review and asks
//! `yaygui_core::plan` for the commands. Serialization matches
//! `apps/desktop/src/api/types.ts`.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use yaygui_aur_client::Package as AurPackage;
use yaygui_core::{
    PackageName, PackageRef, PlanContext, PlanError, PlannedCommand, QueueItem, RemoveMode,
    ReviewMap, ReviewStatus, Source,
};
use yaygui_review::{Approval, review_status};

/// Upper bound on AUR packages (queued plus dependencies) in one plan.
pub const MAX_AUR_PACKAGES: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum SourceDto {
    Repo { repo: String },
    Aur,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum QueueItemDto {
    Install { name: String, source: SourceDto },
    Remove { name: String, mode: RemoveMode },
    SystemUpgrade { devel: bool },
    CleanOrphans,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanErrorKind {
    Invalid,
    NotInRepos,
    NotInAur,
    AurUnavailable,
    PartialUpgrade,
    Unreviewed,
    Conflict,
    NotInstallable,
    TooManyAurPackages,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanErrorDto {
    pub kind: PlanErrorKind,
    pub message: String,
    pub packages: Vec<String>,
}

impl PlanErrorDto {
    pub fn new(kind: PlanErrorKind, message: impl Into<String>, packages: Vec<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            packages,
        }
    }
}

impl From<PlanError> for PlanErrorDto {
    fn from(e: PlanError) -> Self {
        let names = |v: &[PackageName]| v.iter().map(|n| n.as_str().to_owned()).collect();
        let (kind, packages) = match &e {
            PlanError::PartialUpgrade(v) => (PlanErrorKind::PartialUpgrade, names(v)),
            PlanError::Unreviewed(v) => (PlanErrorKind::Unreviewed, names(v)),
            PlanError::Conflict(n) => (PlanErrorKind::Conflict, vec![n.as_str().to_owned()]),
            PlanError::NotInstallable(n) => {
                (PlanErrorKind::NotInstallable, vec![n.as_str().to_owned()])
            }
        };
        Self::new(kind, e.to_string(), packages)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandDto {
    pub description: String,
    /// Program followed by its arguments, exactly as it will be spawned.
    pub argv: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    NotReviewed,
    Approved,
    ChangedSinceReview,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewItem {
    pub name: String,
    pub pkgbase: String,
    pub version: String,
    pub status: ReviewState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved_commit: Option<String>,
    /// Set for AUR dependencies that were not queued themselves.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependency_of: Option<String>,
    pub maintainer: Option<String>,
    pub last_modified: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanResponse {
    pub commands: Vec<CommandDto>,
    pub reviews: Vec<ReviewItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<PlanErrorDto>,
    /// `/var/lib/pacman/db.lck` exists: another package manager is running (or crashed).
    pub lock_held: bool,
    /// The session-wide "skip review" opt-in was used for this plan.
    pub skip_review: bool,
    pub can_run: bool,
}

/// Everything `start_transaction` needs besides the response.
#[derive(Debug, Default)]
pub struct PlanOutcome {
    pub response: PlanResponse,
    pub commands: Vec<PlannedCommand>,
    /// pkgbase -> approved commit, for the review-pinning overlay.
    pub approved: BTreeMap<String, String>,
}

impl PlanOutcome {
    pub fn error(e: PlanErrorDto) -> Self {
        Self {
            response: PlanResponse {
                error: Some(e),
                ..PlanResponse::default()
            },
            ..Self::default()
        }
    }
}

/// Validates names and converts the queue.
pub fn parse_items(items: Vec<QueueItemDto>) -> Result<Vec<QueueItem>, PlanErrorDto> {
    let parse = |s: String| {
        s.parse::<PackageName>().map_err(|e| {
            PlanErrorDto::new(
                PlanErrorKind::Invalid,
                format!("{s:?}: {e}"),
                vec![s.clone()],
            )
        })
    };
    items
        .into_iter()
        .map(|i| {
            Ok(match i {
                QueueItemDto::Install { name, source } => QueueItem::Install(PackageRef {
                    name: parse(name)?,
                    source: match source {
                        SourceDto::Repo { repo } => Source::Repo(repo),
                        SourceDto::Aur => Source::Aur,
                    },
                }),
                QueueItemDto::Remove { name, mode } => QueueItem::Remove {
                    name: parse(name)?,
                    mode,
                },
                QueueItemDto::SystemUpgrade { devel } => QueueItem::SystemUpgrade { devel },
                QueueItemDto::CleanOrphans => QueueItem::CleanOrphans,
            })
        })
        .collect()
}

/// Names of queued installs with the given source kind.
pub fn installs(queue: &[QueueItem], aur: bool) -> Vec<String> {
    queue
        .iter()
        .filter_map(|i| match i {
            QueueItem::Install(p) if (p.source == Source::Aur) == aur => {
                Some(p.name.as_str().to_owned())
            }
            _ => None,
        })
        .collect()
}

/// Strips a version constraint: `foo>=1.2` -> `foo`.
pub fn dep_name(dep: &str) -> &str {
    dep.split(['<', '>', '=', ':']).next().unwrap_or(dep).trim()
}

/// Dependencies of `pkg` that the repos or the local system do not satisfy
/// by name, so yay will look for them in the AUR. Provides are not
/// resolved; an unknown virtual name is simply not found in the AUR.
pub fn unresolved_deps(pkg: &AurPackage, satisfied: impl Fn(&str) -> bool) -> Vec<String> {
    let mut out: Vec<String> = pkg
        .depends
        .iter()
        .chain(&pkg.make_depends)
        .chain(&pkg.check_depends)
        .map(|d| dep_name(d).to_owned())
        .filter(|d| !d.is_empty() && !satisfied(d))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Facts gathered by the caller (IO) for [`assemble`].
pub struct Facts<'a> {
    /// AUR info for every queued AUR package and every AUR dependency found.
    pub aur: &'a HashMap<String, AurPackage>,
    /// dependency name -> the queued package that pulled it in.
    pub dependency_of: &'a HashMap<String, String>,
    /// Latest approval per pkgbase.
    pub latest: &'a dyn Fn(&str) -> Option<Approval>,
    pub skip_review: bool,
    pub lock_held: bool,
}

/// Builds the plan from validated queue items and gathered facts.
pub fn assemble(queue: &[QueueItem], facts: &Facts<'_>) -> PlanOutcome {
    let queued_aur = installs(queue, true);
    let missing: Vec<String> = queued_aur
        .iter()
        .filter(|n| !facts.aur.contains_key(*n))
        .cloned()
        .collect();
    if !missing.is_empty() {
        return PlanOutcome::error(PlanErrorDto::new(
            PlanErrorKind::NotInAur,
            format!("not found in the AUR: {}", missing.join(", ")),
            missing,
        ));
    }

    let mut reviews = Vec::new();
    let mut review_map = ReviewMap::new();
    let mut approved = BTreeMap::new();
    let mut seen = HashSet::new();
    let deps = {
        let mut d: Vec<&String> = facts.dependency_of.keys().collect();
        d.sort();
        d
    };
    for name in queued_aur.iter().chain(deps) {
        let Some(pkg) = facts.aur.get(name) else {
            continue;
        };
        if !seen.insert(name.clone()) {
            continue;
        }
        let latest = (facts.latest)(&pkg.package_base);
        let status = review_status(latest.as_ref(), None, Some(pkg.last_modified));
        let (state, commit) = match &status {
            ReviewStatus::Approved { commit } => {
                approved.insert(pkg.package_base.clone(), commit.clone());
                (ReviewState::Approved, Some(commit.clone()))
            }
            ReviewStatus::NotReviewed => (ReviewState::NotReviewed, None),
            ReviewStatus::ChangedSinceReview => (ReviewState::ChangedSinceReview, None),
        };
        if let Ok(n) = name.parse::<PackageName>() {
            review_map.insert(n, status);
        }
        reviews.push(ReviewItem {
            name: name.clone(),
            pkgbase: pkg.package_base.clone(),
            version: pkg.version.clone(),
            status: state,
            approved_commit: commit,
            dependency_of: facts.dependency_of.get(name).cloned(),
            maintainer: pkg.maintainer.clone(),
            last_modified: pkg.last_modified,
        });
    }

    let ctx = PlanContext {
        reviews: review_map,
        system_up_to_date: false,
        skip_review_session_opt_in: facts.skip_review,
    };
    let mut error = core_plan_error(queue, &ctx);
    let commands = match yaygui_core::plan(queue, &ctx) {
        Ok(c) => c,
        Err(e) => {
            error.get_or_insert(e.into());
            Vec::new()
        }
    };
    // The core planner only knows queued packages; dependencies need a review too.
    if error.is_none() && !facts.skip_review {
        let bad: Vec<String> = reviews
            .iter()
            .filter(|r| r.status != ReviewState::Approved)
            .map(|r| r.name.clone())
            .collect();
        if !bad.is_empty() {
            error = Some(PlanErrorDto::new(
                PlanErrorKind::Unreviewed,
                format!("AUR packages not reviewed: {}", bad.join(", ")),
                bad,
            ));
        }
    }
    let can_run = error.is_none() && !commands.is_empty() && !facts.lock_held;
    PlanOutcome {
        response: PlanResponse {
            commands: commands
                .iter()
                .map(|c| CommandDto {
                    description: c.description.clone(),
                    argv: std::iter::once(c.program.clone())
                        .chain(c.args.iter().cloned())
                        .collect(),
                })
                .collect(),
            reviews,
            error,
            lock_held: facts.lock_held,
            skip_review: facts.skip_review,
            can_run,
        },
        commands: if can_run { commands } else { Vec::new() },
        approved: if facts.skip_review {
            BTreeMap::new()
        } else {
            approved
        },
    }
}

/// Partial-upgrade and conflict errors take precedence over review errors so
/// the user fixes the queue before reviewing.
fn core_plan_error(queue: &[QueueItem], ctx: &PlanContext) -> Option<PlanErrorDto> {
    let relaxed = PlanContext {
        skip_review_session_opt_in: true,
        ..ctx.clone()
    };
    yaygui_core::plan(queue, &relaxed).err().map(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const C1: &str = "1111111111111111111111111111111111111111";

    fn aur_pkg(name: &str, base: &str, depends: &[&str], modified: i64) -> AurPackage {
        serde_json::from_value(json!({
            "Name": name, "PackageBase": base, "Version": "1-1",
            "Maintainer": "m", "LastModified": modified, "Depends": depends,
        }))
        .unwrap()
    }

    fn items(v: serde_json::Value) -> Vec<QueueItem> {
        parse_items(serde_json::from_value(v).unwrap()).unwrap()
    }

    fn approval(at: i64) -> Option<Approval> {
        Some(Approval {
            commit: C1.into(),
            approved_at: at,
            maintainer: None,
        })
    }

    fn facts<'a>(
        aur: &'a HashMap<String, AurPackage>,
        deps: &'a HashMap<String, String>,
        latest: &'a dyn Fn(&str) -> Option<Approval>,
    ) -> Facts<'a> {
        Facts {
            aur,
            dependency_of: deps,
            latest,
            skip_review: false,
            lock_held: false,
        }
    }

    #[test]
    fn dto_shapes_parse() {
        let q = items(json!([
            {"kind": "install", "name": "vim", "source": {"kind": "repo", "repo": "extra"}},
            {"kind": "install", "name": "foo", "source": {"kind": "aur"}},
            {"kind": "remove", "name": "bar", "mode": "with_deps"},
            {"kind": "system_upgrade", "devel": true},
            {"kind": "clean_orphans"}
        ]));
        assert_eq!(q.len(), 5);
        assert_eq!(installs(&q, false), ["vim"]);
        assert_eq!(installs(&q, true), ["foo"]);
    }

    #[test]
    fn invalid_name_rejected() {
        let dto = serde_json::from_value(json!([
            {"kind": "install", "name": "--evil", "source": {"kind": "aur"}}
        ]))
        .unwrap();
        let e = parse_items(dto).unwrap_err();
        assert_eq!(e.kind, PlanErrorKind::Invalid);
    }

    #[test]
    fn dep_names_and_unresolved() {
        assert_eq!(dep_name("foo>=1.2"), "foo");
        assert_eq!(dep_name("bar"), "bar");
        let p = aur_pkg("a", "a", &["glibc", "libfoo>=2", "libbar"], 0);
        assert_eq!(unresolved_deps(&p, |d| d == "glibc"), ["libbar", "libfoo"]);
    }

    #[test]
    fn approved_aur_runs_with_pinning() {
        let aur = HashMap::from([("foo".to_owned(), aur_pkg("foo", "foo-base", &[], 50))]);
        let deps = HashMap::new();
        let latest = |b: &str| (b == "foo-base").then(|| approval(100)).flatten();
        let q = items(json!([{"kind": "install", "name": "foo", "source": {"kind": "aur"}}]));
        let out = assemble(&q, &facts(&aur, &deps, &latest));
        assert_eq!(out.response.error, None);
        assert!(out.response.can_run);
        assert_eq!(out.approved.get("foo-base").map(String::as_str), Some(C1));
        assert_eq!(
            out.response.commands[0].argv,
            [
                "yay",
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
        let j = serde_json::to_value(&out.response).unwrap();
        assert_eq!(j["reviews"][0]["status"], "approved");
        assert_eq!(j["lockHeld"], false);
        assert_eq!(j["canRun"], true);
    }

    #[test]
    fn modified_after_approval_needs_new_review() {
        let aur = HashMap::from([("foo".to_owned(), aur_pkg("foo", "foo", &[], 200))]);
        let deps = HashMap::new();
        let latest = |_: &str| approval(100);
        let q = items(json!([{"kind": "install", "name": "foo", "source": {"kind": "aur"}}]));
        let out = assemble(&q, &facts(&aur, &deps, &latest));
        assert_eq!(
            out.response.reviews[0].status,
            ReviewState::ChangedSinceReview
        );
        assert_eq!(out.response.error.unwrap().kind, PlanErrorKind::Unreviewed);
        assert!(out.commands.is_empty());
    }

    #[test]
    fn unreviewed_dependency_blocks() {
        let aur = HashMap::from([
            ("foo".to_owned(), aur_pkg("foo", "foo", &["libx"], 0)),
            ("libx".to_owned(), aur_pkg("libx", "libx", &[], 0)),
        ]);
        let deps = HashMap::from([("libx".to_owned(), "foo".to_owned())]);
        let latest = |b: &str| (b == "foo").then(|| approval(100)).flatten();
        let q = items(json!([{"kind": "install", "name": "foo", "source": {"kind": "aur"}}]));
        let out = assemble(&q, &facts(&aur, &deps, &latest));
        let e = out.response.error.unwrap();
        assert_eq!(e.kind, PlanErrorKind::Unreviewed);
        assert_eq!(e.packages, ["libx"]);
        assert_eq!(
            out.response.reviews[1].dependency_of.as_deref(),
            Some("foo")
        );
        assert!(!out.response.can_run);
    }

    #[test]
    fn partial_upgrade_reported_before_reviews() {
        let aur = HashMap::from([("foo".to_owned(), aur_pkg("foo", "foo", &[], 0))]);
        let deps = HashMap::new();
        let latest = |_: &str| None;
        let q = items(json!([
            {"kind": "install", "name": "vim", "source": {"kind": "repo", "repo": "extra"}},
            {"kind": "install", "name": "foo", "source": {"kind": "aur"}}
        ]));
        let out = assemble(&q, &facts(&aur, &deps, &latest));
        assert_eq!(
            out.response.error.unwrap().kind,
            PlanErrorKind::PartialUpgrade
        );
    }

    #[test]
    fn missing_aur_package() {
        let aur = HashMap::new();
        let deps = HashMap::new();
        let latest = |_: &str| None;
        let q = items(json!([{"kind": "install", "name": "ghost", "source": {"kind": "aur"}}]));
        let out = assemble(&q, &facts(&aur, &deps, &latest));
        assert_eq!(out.response.error.unwrap().kind, PlanErrorKind::NotInAur);
    }

    #[test]
    fn lock_and_skip_review() {
        let aur = HashMap::from([("foo".to_owned(), aur_pkg("foo", "foo", &[], 0))]);
        let deps = HashMap::new();
        let latest = |_: &str| None;
        let q = items(json!([{"kind": "install", "name": "foo", "source": {"kind": "aur"}}]));
        let mut f = facts(&aur, &deps, &latest);
        f.skip_review = true;
        f.lock_held = true;
        let out = assemble(&q, &f);
        assert_eq!(out.response.error, None);
        assert!(!out.response.can_run, "lock blocks running");
        assert!(out.approved.is_empty());
        assert_eq!(
            out.response.commands[0].argv,
            ["yay", "-S", "--needed", "--", "foo"]
        );
    }
}
