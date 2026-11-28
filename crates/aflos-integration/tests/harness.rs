//! The harness on the echo integration (PLAN-integrations Task 3): the real
//! one passes; broken ones fail with a message that says what's wrong.

#![cfg(feature = "host")]

use aflos_integration::harness::{self, Check, Outcome};
use std::process::Command;

const ECHO: &str = env!("CARGO_BIN_EXE_aflos-echo");

fn failures(checks: &[Check]) -> Vec<String> {
    checks
        .iter()
        .filter_map(|c| match &c.outcome {
            Outcome::Failed(why) => Some(format!("{}: {why}", c.name)),
            _ => None,
        })
        .collect()
}

/// Runs echo in `mode` through the harness. The mode reaches echo through
/// the environment, so each test runs the harness binary as its own process.
fn harness(mode: &str) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_aflos-integration-test"))
        .arg(ECHO)
        .env("AFLOS_ECHO_MODE", mode)
        .output()
        .unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn echo_passes_every_check() {
    let checks = harness::check(ECHO.as_ref(), &[]).await;
    assert_eq!(failures(&checks), Vec::<String>::new());
    let names: Vec<_> = checks.iter().map(|c| c.name).collect();
    for want in [
        "answers hello",
        "its rail tile",
        "answers ping",
        "signs in",
        "lists its chats",
        "sends a message",
        "reacts",
        "takes its settings",
        "quits when asked",
    ] {
        assert!(names.contains(&want), "{want} wasn't checked: {names:?}");
    }
}

#[test]
fn the_binary_says_all_passed() {
    let (ok, out) = harness("");
    assert!(ok, "{out}");
    assert!(out.contains("all passed"), "{out}");
}

#[test]
fn a_broken_echo_fails_with_a_clear_message() {
    let (ok, out) = harness("broken");
    assert!(!ok, "{out}");
    assert!(
        out.contains("FAILED  sends a message: it says it can send, but the chat never showed the sent message"),
        "{out}"
    );
}

#[test]
fn one_that_crashes_fails_at_hello() {
    let (ok, out) = harness("crash");
    assert!(!ok, "{out}");
    assert!(
        out.contains("FAILED  answers hello: it crashed (exit code 3)"),
        "{out}"
    );
}

#[test]
fn one_from_the_future_says_so() {
    let (ok, out) = harness("newer");
    assert!(!ok, "{out}");
    assert!(out.contains("needs a newer Aflos"), "{out}");
}
