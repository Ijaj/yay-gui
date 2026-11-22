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

/// Answers for yay `==>` menus: `(text the menu contains, line to type)`.
pub const MENUS: &[(&str, &str)] = &[
    // "Packages to cleanBuild?" with options "[N]one [A]ll [Ab]ort [I]nstalled [No]tInstalled":
    // nothing needs a clean build in a fresh container.
    ("packages to cleanbuild", "N"),
];

/// Prompts `PromptDetector` does not recognise today (found by running real
/// yay): `(text the screen contains, line to type)`. They are matched on the
/// ANSI-stripped screen tail only when it ends in a bare `==>` and the output
/// has been silent for a moment. Remove an entry once the detector emits it.
pub const RAW_MENUS: &[(&str, &str)] = &[
    // yay -Syu: "==> Packages to exclude: (eg: "1 2 3", "1-3", "^4" or repo name)".
    // Empty answer = exclude nothing, which keeps the upgrade complete.
    ("packages to exclude", ""),
];

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

/// Looks up the line to type for a menu the detector does not report, from the
/// screen tail.
#[must_use]
pub fn raw_menu(tail: &str) -> Option<&'static str> {
    let t = tail.trim_end();
    if !t.ends_with("==>") {
        return None;
    }
    let lower = t.to_lowercase();
    RAW_MENUS
        .iter()
        .find(|(needle, _)| lower.contains(needle))
        .map(|(_, a)| *a)
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
    fn raw_menu_needs_prompt_at_the_end() {
        let screen = "==> Packages to exclude: (eg: \"1 2 3\")\n -> Excluding packages may cause partial upgrades\n==> ";
        assert_eq!(raw_menu(screen), Some(""));
        assert_eq!(raw_menu("==> Packages to exclude: x\nbuilding..."), None);
        assert_eq!(raw_menu("==> Something else\n==> "), None);
    }

    #[test]
    fn menus() {
        assert_eq!(menu("==> Packages to cleanBuild?\n[N]one"), Some("N"));
        assert_eq!(menu("==> Diffs to show?"), None);
    }
}
