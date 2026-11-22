//! Watches the raw terminal output for stalls. Menus are reported by
//! `PromptDetector` (`PromptEvent::Menu`) and answered through the prompt
//! table; this only catches a bare `==>` menu the detector did not report, and
//! a command that goes silent. It never answers anything.

use std::time::Duration;

use crate::prompts::strip_ansi;

/// Silence after which the screen is inspected.
pub const SETTLE: Duration = Duration::from_secs(2);
/// No output for this long and nothing recognised: stuck on an unknown prompt.
pub const IDLE_LIMIT: Duration = Duration::from_secs(120);
const TAIL_BYTES: usize = 4096;

/// What to do after a quiet moment.
#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    /// Nothing to do yet.
    Nothing,
    /// Fail the scenario with this reason.
    Fail(String),
}

/// Screen tail plus the bookkeeping that keeps a handled screen from failing.
#[derive(Default)]
pub struct Screen {
    tail: Vec<u8>,
    total: usize,
    /// `total` at the time something already dealt with the current screen.
    handled_at: Option<usize>,
}

impl Screen {
    /// A new command started.
    pub fn command_started(&mut self) {
        self.tail.clear();
        self.handled_at = None;
    }

    /// Raw bytes from the PTY.
    pub fn output(&mut self, bytes: &[u8]) {
        self.total += bytes.len();
        self.tail.extend_from_slice(bytes);
        if self.tail.len() > TAIL_BYTES {
            let cut = self.tail.len() - TAIL_BYTES;
            self.tail.drain(..cut);
        }
    }

    /// The detector reported a prompt for the current screen (and the caller
    /// answers, ignores or fails it), so this screen needs no further attention.
    pub fn prompt_reported(&mut self) {
        self.handled_at = Some(self.total);
    }

    /// Called when no event arrived for `idle`.
    pub fn poll(&mut self, idle: Duration) -> Decision {
        if idle < SETTLE || self.handled_at == Some(self.total) {
            return Decision::Nothing;
        }
        let screen = strip_ansi(&self.tail);
        let lines: Vec<&str> = screen
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        if lines.last() == Some(&"==>") {
            let n = lines.len();
            let shown = lines[n.saturating_sub(4)..].join(" | ");
            return Decision::Fail(format!("unrecognised yay menu, not answering: {shown:?}"));
        }
        if idle >= IDLE_LIMIT {
            let shown = lines[lines.len().saturating_sub(3)..].join(" | ");
            return Decision::Fail(format!(
                "no output for {} s; probably an unrecognised prompt. Screen ends with: {shown:?}",
                idle.as_secs()
            ));
        }
        Decision::Nothing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUIET: Duration = Duration::from_secs(3);

    fn screen(text: &str) -> Screen {
        let mut s = Screen::default();
        s.command_started();
        s.output(text.as_bytes());
        s
    }

    #[test]
    fn an_unreported_bare_menu_fails() {
        let mut s = screen("==> Diffs to show?\n==> [N]one [A]ll\n==> ");
        assert_eq!(s.poll(Duration::from_secs(1)), Decision::Nothing);
        assert!(matches!(s.poll(QUIET), Decision::Fail(_)));
    }

    #[test]
    fn menus_the_detector_reported_are_left_alone() {
        let mut s = screen("==> Packages to cleanBuild?\n==> [N]one [A]ll\n==> ");
        s.prompt_reported();
        assert_eq!(s.poll(QUIET), Decision::Nothing);
        s.output(b"N\r\n");
        assert_eq!(s.poll(QUIET), Decision::Nothing);
    }

    #[test]
    fn long_silence_fails() {
        let mut s = screen("compiling things\n");
        assert_eq!(s.poll(QUIET), Decision::Nothing);
        assert!(matches!(s.poll(IDLE_LIMIT), Decision::Fail(_)));
    }
}
