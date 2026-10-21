//! Library consumers receive outcomes without parsing terminal output.

use rustbase_updater::{
    App, Backend, Release, State, Transaction,
    cli::{self, Operation, Report},
    outcome::{Admission, Error, Outcome, Stage},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Policy {
    signers: String,
}

fn policy() -> Policy {
    Policy {
        signers: "/etc/demo/release-signers".into(),
    }
}

/// Registers a profile name, as an app hosting several profiles would.
struct Demo;
impl App for Demo {
    type Policy = Policy;
    const ADAPTER: &'static str = "fixture-v1";
    const USAGE: &'static str =
        "usage: demo-update [--json] [--profile demo] poll | status";
    const PROFILE: Option<&'static str> = Some("demo");
}

/// Registers no profile, so the option is not accepted at all.
struct Single;
impl App for Single {
    type Policy = Policy;
    const ADAPTER: &'static str = "single-v1";
    const USAGE: &'static str = "usage: single-update [--json] poll";
}

fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(|&w| w.to_owned()).collect()
}

fn release(tag: &str) -> Release {
    Release::new(tag, &"a".repeat(40)).unwrap()
}
struct Host {
    state: State,
    current: Release,
    admission: Admission,
    calls: Vec<&'static str>,
}
impl Host {
    fn new(admission: Admission) -> Self {
        Self {
            state: State::default(),
            current: release("v0.1.1"),
            admission,
            calls: vec![],
        }
    }
}
impl Backend for Host {
    fn load(&mut self) -> Result<State, String> {
        self.calls.push("load");
        Ok(self.state.clone())
    }
    fn save(&mut self, state: &State) -> Result<(), String> {
        self.calls.push("save");
        self.state = state.clone();
        Ok(())
    }
    fn current(&mut self) -> Result<Release, String> {
        Ok(self.current.clone())
    }
    fn refresh(&mut self) -> Result<Vec<String>, String> {
        self.calls.push("fetch");
        Ok(vec!["v0.2.0".into()])
    }
    fn admit(&mut self, _: &str) -> Result<Option<Release>, String> {
        Ok(self.admission.release().cloned())
    }
    fn inspect(&mut self, _: &str) -> Result<Admission, String> {
        Ok(self.admission.clone())
    }
    fn build(&mut self, _: &Release) -> Result<(), String> {
        self.calls.push("build");
        Ok(())
    }
    fn activate(&mut self, release: &Release) -> Result<(), String> {
        self.current = release.clone();
        Ok(())
    }
    fn healthy(&mut self, _: &Release) -> Result<(), String> {
        Ok(())
    }
}
fn execute(host: &mut Host, operation: &Operation) -> Report<Policy> {
    cli::execute::<Demo>(operation, host, &policy()).unwrap()
}

#[test]
fn stable_structured_reports_cover_waiting_rejection_activation_and_recovery() {
    for admission in [
        Admission::Waiting {
            reason: "checks running".into(),
        },
        Admission::Rejected {
            reason: "untrusted".into(),
        },
    ] {
        let mut host = Host::new(admission.clone());
        let Report::Polled {
            result:
                Outcome::Unchanged {
                    current,
                    not_admitted,
                },
        } = execute(&mut host, &Operation::Poll)
        else {
            panic!("unchanged expected")
        };
        assert_eq!(current.tag(), "v0.1.1");
        assert_eq!(not_admitted[0].admission, admission);
        assert!(!host.calls.contains(&"build"));
    }
    let candidate = release("v0.2.0");
    let mut host = Host::new(Admission::Eligible {
        release: candidate.clone(),
    });
    let report = execute(&mut host, &Operation::Poll);
    assert!(
        matches!(&report, Report::Polled { result: Outcome::Activated { current, .. } } if current == &candidate)
    );
    assert!(cli::human(&report).unwrap().contains("Activated v0.2.0"));
    host.state.pending = Some(Transaction {
        previous: release("v0.1.1"),
        candidate,
    });
    host.state.blocked.insert("v0.2.0".into());
    host.calls.clear();
    let report = execute(&mut host, &Operation::Poll);
    assert!(
        matches!(&report, Report::Polled { result: Outcome::Recovered { current, .. } } if current.tag() == "v0.1.1")
    );
    assert!(!host.calls.contains(&"fetch"));
    assert!(cli::human(&report).unwrap().contains("Restored v0.1.1"));
}

#[test]
fn status_inspection_and_retry_do_not_build_or_activate() {
    let mut host = Host::new(Admission::Eligible {
        release: release("v0.2.0"),
    });
    let report = execute(&mut host, &Operation::Status);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            &cli::machine(&report).unwrap()
        )
        .unwrap()["schema"],
        1
    );
    assert_eq!(
        serde_json::to_value(&report).unwrap()["adapter"],
        "fixture-v1"
    );
    assert!(
        serde_json::from_str::<serde_json::Value>(
            &cli::human(&report).unwrap()
        )
        .unwrap()
        .get("blocked")
        .is_some()
    );
    assert_eq!(host.calls, ["load"]);
    let inspected = execute(&mut host, &Operation::Inspect("v0.2.0".into()));
    assert!(cli::human(&inspected).unwrap().contains("Eligible v0.2.0"));
    host.admission = Admission::NotAdmitted;
    assert!(
        cli::human(&execute(&mut host, &Operation::Inspect("v0.2.0".into())))
            .is_err()
    );
    host.state.blocked.insert("v0.2.0".into());
    let retried = execute(&mut host, &Operation::Retry("v0.2.0".into()));
    assert!(!host.state.blocked.contains("v0.2.0"));
    assert!(
        cli::human(&retried)
            .unwrap()
            .contains("admission remains required")
    );
    assert!(!host.calls.contains(&"build"));
    assert_eq!(
        cli::human(&execute(&mut host, &Operation::Help)).unwrap(),
        Demo::USAGE
    );
    let status =
        cli::execute::<Single>(&Operation::Status, &mut host, &policy())
            .unwrap();
    let status = serde_json::to_value(&status).unwrap();
    assert_eq!(status["adapter"], "single-v1");
    assert_eq!(status["policy"]["signers"], "/etc/demo/release-signers");
}

#[test]
fn malformed_requests_and_wrong_admitted_tag_fail_before_build() {
    for words in [
        &[][..],
        &["--profile"],
        &["--profile", "other", "status"],
        &["inspect", "../bad"],
        &["retry", "--force"],
        &["unknown"],
    ] {
        assert!(cli::parse::<Demo>(&args(words)).is_err(), "{words:?}");
    }
    for words in [
        &["status"][..],
        &["--json", "--profile", "demo", "poll"],
        &["inspect", "v0.2.0"],
        &["retry", "v0.2.0"],
        &["--help"],
    ] {
        assert!(cli::parse::<Demo>(&args(words)).is_ok(), "{words:?}");
    }
    let mut host = Host::new(Admission::Eligible {
        release: release("v0.3.0"),
    });
    let error = cli::execute::<Demo>(&Operation::Poll, &mut host, &policy())
        .unwrap_err();
    assert_eq!(error.stage, Stage::Admission);
    assert!(!host.calls.contains(&"build"));
    let mut error = Error::new(Stage::Build, "failed");
    error.restored = Some(Box::new(release("v0.1.1")));
    error.recovery = Some(Box::new(Error::new(Stage::Recovery, "unavailable")));
    assert!(
        error
            .to_string()
            .contains("restored v0.1.1; recovery failed")
    );
}

#[test]
fn an_app_without_a_profile_refuses_the_option_and_uses_its_usage() {
    for words in [&["--profile", "demo", "poll"][..], &["--profile"]] {
        let error = cli::parse::<Single>(&args(words)).unwrap_err();
        assert_eq!(error.stage, Stage::Configuration, "{words:?}");
    }
    let error = cli::parse::<Single>(&args(&["bogus"])).unwrap_err();
    assert_eq!(
        (error.stage, error.message.as_str()),
        (Stage::Usage, Single::USAGE)
    );
    assert!(cli::parse::<Single>(&args(&["--json", "poll"])).is_ok());
}
