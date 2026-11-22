//! The explicit table of prompts the harness answers. Anything else fails the
//! scenario: the harness never answers a question it does not recognise.

use aurdeck_yay_runner::MenuKind;

/// Answers for `[Y/n]` and `[y/N]` questions: `(text the question contains, answer)`.
/// Matching is case-insensitive on the question text without the `::` marker.
pub const YES_NO: &[(&str, bool)] = &[
    ("proceed with installation", true),
    ("proceed with install", true),
    ("do you want to remove these packages", true),
    ("proceed with removal", true),
];

/// Answers for yay `==>` menus the detector reports, by kind. Nothing needs a
/// clean build in a fresh container. `UpgradeExclude` is answered per scenario
/// (see [`EXCLUDE_AUR_UPGRADES`] and [`EXCLUDE_NOTHING`]); `Diffs` and `Edit`
/// are never answered here (see [`MenuAction`]).
pub const MENUS: &[(MenuKind, &str)] = &[(MenuKind::CleanBuild, "N")];

/// Answer to yay's `==> Packages to exclude:` menu (`-Syu`) in repo scenarios.
/// `aur` names the repository of every AUR package, so all pending AUR
/// upgrades are excluded: a repo scenario must never build an AUR package,
/// reviewed or not. Verified against yay v13.1.0 with an outdated AUR package
/// installed: `aur` leaves it alone ("there is nothing to do"), an empty
/// answer upgrades it.
pub const EXCLUDE_AUR_UPGRADES: &str = "aur";

/// Empty answer to the exclude menu: nothing is excluded, so pending AUR
/// upgrades are really attempted (`pin-unreviewed-upgrade`).
pub const EXCLUDE_NOTHING: &str = "";

/// Bound on the exclude menu: it is answered at most once per transaction; a
/// repeat means yay is looping on it and fails the scenario.
#[derive(Debug, Default)]
pub struct ExcludeOnce(bool);

impl ExcludeOnce {
    /// True the first time, false for every later exclude menu.
    pub fn allow(&mut self) -> bool {
        !std::mem::replace(&mut self.0, true)
    }
}

/// What to do with a menu the detector reported.
#[derive(Debug, PartialEq, Eq)]
pub enum MenuAction {
    /// Type this line (plus Enter).
    Answer(&'static str),
    /// yay answers this one itself (the plan passed the matching flag).
    Ignore,
    /// Not recognised: fail the scenario.
    Unknown,
}

/// Decides what to do with a menu. `args` are the arguments of the running
/// command; `exclude` is the scenario's answer to the exclude menu.
#[must_use]
pub fn menu_action(kind: MenuKind, args: &[String], exclude: &'static str) -> MenuAction {
    let has = |flag: &str| args.iter().any(|a| a == flag);
    match kind {
        MenuKind::UpgradeExclude => MenuAction::Answer(exclude),
        MenuKind::Diffs if has("--answerdiff") => MenuAction::Ignore,
        MenuKind::Edit if has("--answeredit") => MenuAction::Ignore,
        MenuKind::CleanBuild if has("--answerclean") => MenuAction::Ignore,
        MenuKind::CleanBuild => MENUS
            .iter()
            .find(|(k, _)| *k == kind)
            .map_or(MenuAction::Unknown, |(_, a)| MenuAction::Answer(a)),
        MenuKind::Diffs | MenuKind::Edit | MenuKind::Other => MenuAction::Unknown,
    }
}

/// Removes ANSI escape sequences (CSI and OSC) for matching.
#[must_use]
pub fn strip_ansi(bytes: &[u8]) -> String {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != 0x1b {
            out.push(bytes[i]);
            i += 1;
            continue;
        }
        match bytes.get(i + 1) {
            Some(b'[') => {
                i += 2;
                while i < bytes.len() && !(0x40..=0x7e).contains(&bytes[i]) {
                    i += 1;
                }
                i += 1;
            }
            Some(b']') => {
                i += 2;
                while i < bytes.len() && bytes[i] != 0x07 && bytes[i] != 0x1b {
                    i += 1;
                }
                // BEL, or ESC followed by the string terminator backslash
                i += if bytes.get(i) == Some(&0x1b) { 2 } else { 1 };
            }
            _ => i += 2,
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Looks up the answer to a yes/no question.
#[must_use]
pub fn yes_no(question: &str) -> Option<bool> {
    let q = question.to_lowercase();
    YES_NO
        .iter()
        .find(|(needle, _)| q.contains(needle))
        .map(|(_, a)| *a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_questions_are_answered() {
        assert_eq!(yes_no("Proceed with installation?"), Some(true));
        assert_eq!(yes_no("Proceed with install?"), Some(true));
        assert_eq!(yes_no("Do you want to remove these packages?"), Some(true));
    }

    #[test]
    fn unknown_questions_are_not_answered() {
        assert_eq!(yes_no("Import PGP key ABCD?"), None);
        assert_eq!(yes_no("Replace foo with bar?"), None);
        assert_eq!(yes_no(""), None);
    }

    #[test]
    fn strips_ansi() {
        let raw = b"\x1b[1m::\x1b[0m hi\x1b]3008;start=x\x1b\\there \x1b]0;t\x07ok";
        assert_eq!(strip_ansi(raw), ":: hithere ok");
    }

    #[test]
    fn the_exclude_menu_is_answered_once_per_transaction() {
        let mut once = ExcludeOnce::default();
        assert!(once.allow());
        assert!(!once.allow());
        assert!(!once.allow());
        // a new transaction starts fresh
        assert!(ExcludeOnce::default().allow());
    }

    #[test]
    fn menus() {
        let none: Vec<String> = Vec::new();
        let flags: Vec<String> = ["--answerdiff", "None", "--answeredit", "None"]
            .map(String::from)
            .to_vec();
        assert_eq!(
            menu_action(MenuKind::CleanBuild, &none, "aur"),
            MenuAction::Answer("N")
        );
        assert_eq!(
            menu_action(MenuKind::UpgradeExclude, &none, "aur"),
            MenuAction::Answer("aur")
        );
        assert_eq!(
            menu_action(MenuKind::UpgradeExclude, &none, EXCLUDE_NOTHING),
            MenuAction::Answer("")
        );
        assert_eq!(
            menu_action(MenuKind::Diffs, &flags, "aur"),
            MenuAction::Ignore
        );
        assert_eq!(
            menu_action(MenuKind::Edit, &flags, "aur"),
            MenuAction::Ignore
        );
        // without the flag the harness never answers a diff or edit menu
        assert_eq!(
            menu_action(MenuKind::Diffs, &none, "aur"),
            MenuAction::Unknown
        );
        assert_eq!(
            menu_action(MenuKind::Edit, &none, "aur"),
            MenuAction::Unknown
        );
        assert_eq!(
            menu_action(MenuKind::Other, &flags, "aur"),
            MenuAction::Unknown
        );
    }
}
