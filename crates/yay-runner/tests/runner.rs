use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use yaygui_yay_runner::{PromptDetector, PromptEvent, RunHandle, RunSpec, Runner};
use zeroize::Zeroizing;

const T: Duration = Duration::from_secs(10);

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

type Output = Arc<Mutex<Vec<u8>>>;

fn start(spec: RunSpec) -> (RunHandle, Receiver<PromptEvent>, Output) {
    let (tx, rx) = mpsc::channel();
    let out: Output = Arc::default();
    let out2 = Arc::clone(&out);
    let mut det = PromptDetector::new();
    let h = Runner::spawn(spec, move |bytes| {
        out2.lock().unwrap().extend_from_slice(bytes);
        for ev in det.feed(bytes) {
            let _ = tx.send(ev);
        }
    })
    .unwrap();
    (h, rx, out)
}

#[test]
fn full_flow_success() {
    let (h, rx, out) = start(RunSpec::new(fixture("fake-yay.sh"), vec!["0".into()]));
    assert_eq!(
        rx.recv_timeout(T).unwrap(),
        PromptEvent::SudoPassword {
            user: "tester".into()
        }
    );
    h.answer_password(Zeroizing::new("hunter2".to_owned()))
        .unwrap();
    match rx.recv_timeout(T).unwrap() {
        PromptEvent::YesNo {
            question,
            default_yes,
        } => {
            assert_eq!(question, "Proceed with installation?");
            assert!(default_yes);
        }
        other => panic!("{other:?}"),
    }
    h.answer_yes_no(true).unwrap();
    let st = h.wait().unwrap();
    assert!(st.success(), "{st:?}");
    let text = String::from_utf8_lossy(&out.lock().unwrap()).into_owned();
    assert!(text.contains("installing fakepkg"));
    assert!(!text.contains("hunter2"), "password must not be echoed");
}

#[test]
fn nonzero_exit_code_is_reported() {
    let (h, rx, _) = start(RunSpec::new(fixture("fake-yay.sh"), vec!["7".into()]));
    rx.recv_timeout(T).unwrap();
    h.answer_password(Zeroizing::new("hunter2".to_owned()))
        .unwrap();
    rx.recv_timeout(T).unwrap();
    h.write_input(b"\n").unwrap();
    assert_eq!(h.wait().unwrap().code, 7);
}

#[test]
fn wrong_password_detected() {
    let (h, rx, _) = start(RunSpec::new(fixture("fake-yay.sh"), vec![]));
    rx.recv_timeout(T).unwrap();
    h.answer_password(Zeroizing::new("nope".to_owned()))
        .unwrap();
    assert!(matches!(
        rx.recv_timeout(T).unwrap(),
        PromptEvent::Error {
            kind: yaygui_yay_runner::ErrorKind::AuthFailed,
            ..
        }
    ));
    assert_eq!(h.wait().unwrap().code, 3);
}

#[test]
fn kill_terminates_child() {
    let (h, _rx, out) = start(RunSpec::new(fixture("fake-hang.sh"), vec![]));
    for _ in 0..200 {
        if String::from_utf8_lossy(&out.lock().unwrap()).contains("started") {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(h.try_wait().unwrap().is_none());
    h.kill().unwrap();
    let st = h.wait().unwrap();
    assert!(!st.success());
}

#[test]
fn interrupt_sends_sigint() {
    let (h, _rx, out) = start(RunSpec::new(fixture("fake-hang.sh"), vec![]));
    for _ in 0..200 {
        if String::from_utf8_lossy(&out.lock().unwrap()).contains("started") {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    h.interrupt().unwrap();
    assert!(!h.wait().unwrap().success());
}

#[test]
fn env_cwd_args_and_resize() {
    let dir = tempfile::tempdir().unwrap();
    let mut spec = RunSpec::new(
        "sh",
        vec![
            "-c".into(),
            "echo \"$YG_TEST|$1|$(pwd)\"; stty size".into(),
            "sh".into(),
            "a b;c".into(),
        ],
    );
    spec.env.push(("YG_TEST".into(), "val".into()));
    spec.cwd = Some(dir.path().to_owned());
    spec.size = (30, 100);
    let (h, _rx, out) = start(spec);
    assert!(h.wait().unwrap().success());
    h.resize(40, 120).unwrap_or(());
    let text = String::from_utf8_lossy(&out.lock().unwrap()).into_owned();
    assert!(text.contains("val|a b;c|"), "{text}");
    assert!(text.contains("30 100"), "{text}");
}

#[test]
fn spawn_missing_program_fails() {
    assert!(Runner::spawn(RunSpec::new("/nonexistent/prog", vec![]), |_| {}).is_err());
}
