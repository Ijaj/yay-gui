//! The explicit table of prompts the harness answers. Anything else fails the
//! scenario: the harness never answers a question it does not recognise.

/// Answers for `[Y/n]` and `[y/N]` questions: `(text the question contains, answer)`.
/// Matching is case-insensitive on the question text without the `::` marker.
pub const YES_NO: &[(&str, bool)] = &[
    ("proceed with installation", true),
    ("proceed with install", true),
    ("do you want to remove these packages", true),
    ("proceed with removal", true),
];

/// Answers for yay `==>` menus the detector reports: `(text the menu contains, line to type)`.
pub const MENUS: &[(&str, &str)] = &[
    // "Packages to cleanBuild?" with options "[N]one [A]ll [Ab]ort [I]nstalled [No]tInstalled":
    // nothing needs a clean build in a fresh container.
    ("packages to cleanbuild", "N"),
];

/// Answer to yay's `==> Packages to exclude:` menu (`-Syu`), which
/// `PromptDetector` does not report. `aur` names the repository of every AUR
/// package, so all pending AUR upgrades are excluded: a repo scenario must
/// never build an AUR package, reviewed or not. Verified against yay v13.1.0
/// with an outdated AUR package installed: `aur` leaves it alone ("there is
/// nothing to do"), an empty answer upgrades it.
pub const EXCLUDE_AUR_UPGRADES: &str = "aur";

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

/// Looks up the line to type for a menu.
#[must_use]
pub fn menu(text: &str) -> Option<&'static str> {
    let t = text.to_lowercase();
    MENUS
        .iter()
        .find(|(needle, _)| t.contains(needle))
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
    fn menus() {
        assert_eq!(menu("==> Packages to cleanBuild?\n[N]one"), Some("N"));
        assert_eq!(menu("==> Diffs to show?"), None);
    }
}
