//! `start_transaction` with a fake yay.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use yaygui_core::{PlannedCommand, TransactionState};
use yaygui_yay_runner::{PromptEvent, TxConfig, TxEvent, start_transaction};

const T: Duration = Duration::from_secs(10);

fn cmd(last: &str) -> PlannedCommand {
    PlannedCommand {
        program: "yay".into(),
        args: vec!["-S".into(), "--".into(), last.into()],
        description: format!("step {last}"),
    }
}

fn config() -> TxConfig {
    TxConfig {
        program_override: Some(format!(
            "{}/tests/fixtures/fake-tx.sh",
            env!("CARGO_MANIFEST_DIR")
        )),
        ..TxConfig::default()
    }
}

fn states(events: &[TxEvent]) -> Vec<TransactionState> {
    events
        .iter()
        .filter_map(|e| match e {
            TxEvent::State(s) => Some(s.clone()),
            _ => None,
        })
        .collect()
}

fn output(events: &[TxEvent]) -> String {
    let bytes: Vec<u8> = events
        .iter()
        .filter_map(|e| match e {
            TxEvent::Output(b) => Some(b.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

fn collect(rx: &mpsc::Receiver<TxEvent>) -> Vec<TxEvent> {
    let mut v = Vec::new();
    while let Ok(e) = rx.recv_timeout(T) {
        let done = matches!(&e, TxEvent::State(s) if s.is_terminal());
        v.push(e);
        if done {
            break;
        }
    }
    v
}

#[test]
fn runs_all_commands_in_order() {
    let (tx, rx) = mpsc::channel();
    let h = start_transaction(vec![cmd("a"), cmd("b")], config(), move |e| {
        let _ = tx.send(e);
    });
    let ev = collect(&rx);
    assert_eq!(h.wait(), Some(TransactionState::Succeeded));
    assert_eq!(
        states(&ev),
        [TransactionState::Running, TransactionState::Succeeded]
    );
    let started: Vec<(usize, usize, Vec<String>)> = ev
        .iter()
        .filter_map(|e| match e {
            TxEvent::CommandStarted {
                index, total, argv, ..
            } => Some((*index, *total, argv[1..].to_vec())),
            _ => None,
        })
        .collect();
    assert_eq!(started.len(), 2);
    assert_eq!(
        started[1],
        (1, 2, vec!["-S".into(), "--".into(), "b".into()])
    );
    let out = output(&ev);
    let (a, b) = (out.find("done a").unwrap(), out.find("done b").unwrap());
    assert!(a < b, "{out}");
}

#[test]
fn stops_at_first_failure() {
    let (tx, rx) = mpsc::channel();
    let h = start_transaction(vec![cmd("fail"), cmd("b")], config(), move |e| {
        let _ = tx.send(e);
    });
    let ev = collect(&rx);
    let last = h.wait().unwrap();
    match &last {
        TransactionState::Failed { reason } => {
            assert!(
                reason.contains("step fail") && reason.contains("exit code 4"),
                "{reason}"
            );
        }
        other => panic!("{other:?}"),
    }
    assert!(!output(&ev).contains("done b"));
    assert!(
        ev.iter()
            .any(|e| matches!(e, TxEvent::Prompt(PromptEvent::Error { .. })))
    );
}

#[test]
fn prompts_are_answered_through_the_handle() {
    let (tx, rx) = mpsc::channel();
    let h = start_transaction(vec![cmd("ask")], config(), move |e| {
        let _ = tx.send(e);
    });
    let deadline = Instant::now() + T;
    loop {
        assert!(Instant::now() < deadline, "no prompt");
        if let TxEvent::Prompt(PromptEvent::YesNo { .. }) = rx.recv_timeout(T).unwrap() {
            break;
        }
    }
    h.answer_yes_no(true).unwrap();
    assert_eq!(h.wait(), Some(TransactionState::Succeeded));
}

#[test]
fn cancel_stops_running_command_and_skips_the_rest() {
    let (tx, rx) = mpsc::channel();
    let h = start_transaction(vec![cmd("hang"), cmd("b")], config(), move |e| {
        let _ = tx.send(e);
    });
    // Wait until the command is actually running.
    let mut seen = String::new();
    while !seen.contains("building") {
        if let TxEvent::Output(b) = rx.recv_timeout(T).unwrap() {
            seen.push_str(&String::from_utf8_lossy(&b));
        }
    }
    let t0 = Instant::now();
    h.cancel();
    assert_eq!(h.wait(), Some(TransactionState::Cancelled));
    assert!(t0.elapsed() < Duration::from_secs(8), "{:?}", t0.elapsed());
    assert!(!output(&collect(&rx)).contains("done b"));
    assert!(h.answer_yes_no(true).is_err(), "nothing runs any more");
}

#[test]
fn missing_program_fails_cleanly() {
    let cfg = TxConfig {
        program_override: Some("/nonexistent/yay".into()),
        ..TxConfig::default()
    };
    let h = start_transaction(vec![cmd("a")], cfg, |_| {});
    assert!(matches!(h.wait(), Some(TransactionState::Failed { .. })));
}
