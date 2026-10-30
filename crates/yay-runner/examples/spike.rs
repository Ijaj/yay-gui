//! PTY spike: `cargo run -p yaygui-yay-runner --example spike -- yay -S somepkg`
//!
//! Runs the command in a PTY, mirrors its output to stdout, asks for the sudo
//! password on the terminal (no echo) when prompted, and forwards other
//! prompts' answers typed on stdin.

use std::io::{BufRead, Write};
use std::sync::mpsc;

use yaygui_yay_runner::{PromptDetector, PromptEvent, RunSpec, Runner};
use zeroize::Zeroizing;

fn main() {
    let mut argv = std::env::args().skip(1);
    let Some(program) = argv.next() else {
        eprintln!("usage: spike <program> [args...]");
        std::process::exit(2);
    };
    let (tx, rx) = mpsc::channel();
    let mut det = PromptDetector::new();
    let handle = Runner::spawn(RunSpec::new(program, argv.collect()), move |bytes| {
        let mut out = std::io::stdout().lock();
        let _ = out.write_all(bytes);
        let _ = out.flush();
        for ev in det.feed(bytes) {
            let _ = tx.send(ev);
        }
    })
    .expect("spawn failed");

    // Answer prompts on the main thread until the child exits.
    loop {
        match rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(PromptEvent::SudoPassword { user }) => {
                let pw = rpassword::prompt_password(format!("\n[spike] password for {user}: "))
                    .map(Zeroizing::new)
                    .unwrap_or_default();
                handle.answer_password(pw).expect("write password");
            }
            Ok(PromptEvent::YesNo { .. } | PromptEvent::Menu { .. }) => {
                let mut line = String::new();
                if std::io::stdin().lock().read_line(&mut line).is_ok() {
                    let _ = handle.write_input(line.trim_end().as_bytes());
                    let _ = handle.write_input(b"\n");
                }
            }
            Ok(PromptEvent::Error { kind, line }) => {
                eprintln!("\n[spike] detected {kind:?}: {line}")
            }
            Err(_) => {
                if let Ok(Some(_)) = handle.try_wait() {
                    break;
                }
            }
        }
    }
    let status = handle.wait().expect("wait failed");
    eprintln!("\n[spike] exited: {status:?}");
    std::process::exit(i32::try_from(status.code).unwrap_or(1));
}
