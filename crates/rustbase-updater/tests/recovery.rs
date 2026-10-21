//! Upgrade transaction and recovery contracts.
use rustbase_updater::{Backend, Release, State, Transaction, poll, retry};

fn release(tag: &str) -> Release {
    Release::new(tag, &"a".repeat(40)).unwrap()
}

struct Guest {
    state: State,
    current: Release,
    tags: Vec<String>,
    ready: bool,
    failure: &'static str,
    events: Vec<String>,
}

impl Default for Guest {
    fn default() -> Self {
        Self {
            state: State::default(),
            current: release("v0.1.1"),
            tags: vec!["v0.2.0".into()],
            ready: true,
            failure: "",
            events: vec![],
        }
    }
}

impl Backend for Guest {
    fn load(&mut self) -> Result<State, String> {
        Ok(self.state.clone())
    }
    fn save(&mut self, state: &State) -> Result<(), String> {
        if self.failure == "save" {
            return Err("disk full".into());
        }
        self.events.push("save".into());
        self.state = state.clone();
        Ok(())
    }
    fn current(&mut self) -> Result<Release, String> {
        Ok(self.current.clone())
    }
    fn refresh(&mut self) -> Result<Vec<String>, String> {
        self.events.push("fetch".into());
        Ok(self.tags.clone())
    }
    fn admit(&mut self, tag: &str) -> Result<Option<Release>, String> {
        self.events.push(format!("admit {tag}"));
        Ok(self.ready.then(|| release(tag)))
    }
    fn build(&mut self, release: &Release) -> Result<(), String> {
        assert!(
            self.state.pending.is_some(),
            "persist transaction before build"
        );
        self.events.push(format!("build {}", release.tag()));
        if self.failure == "revoked" {
            self.ready = false;
        }
        if self.failure == "build" {
            Err("build failed".into())
        } else {
            Ok(())
        }
    }
    fn activate(&mut self, release: &Release) -> Result<(), String> {
        self.events.push(format!("activate {}", release.tag()));
        self.current = release.clone();
        if (self.failure == "start" && release.tag() == "v0.2.0")
            || self.failure == "rollback"
        {
            Err("start failed".into())
        } else {
            Ok(())
        }
    }
    fn healthy(&mut self, release: &Release) -> Result<(), String> {
        self.events.push(format!("health {}", release.tag()));
        if self.failure == "health" && release.tag() == "v0.2.0" {
            Err("unhealthy".into())
        } else {
            Ok(())
        }
    }
}

#[test]
fn revoked_release_is_rolled_back_and_failed_recovery_remains_durable() {
    let mut guest = Guest {
        failure: "revoked",
        ..Guest::default()
    };
    assert!(poll(&mut guest).is_err());
    assert_eq!(guest.current.tag(), "v0.1.1");
    assert!(!guest.events.iter().any(|s| s == "activate v0.2.0"));
    guest.failure = "rollback";
    guest.state.pending = Some(Transaction {
        previous: release("v0.1.1"),
        candidate: release("v0.2.0"),
    });
    assert!(poll(&mut guest).is_err());
    assert!(guest.state.pending.is_some());
    guest.failure = "";
    poll(&mut guest).unwrap();
    assert!(guest.state.pending.is_none());
}

#[test]
fn persistence_failure_prevents_build_and_invalid_retry_never_changes_state() {
    let mut guest = Guest {
        failure: "save",
        ..Guest::default()
    };
    assert!(poll(&mut guest).is_err());
    assert!(!guest.events.iter().any(|s| s.starts_with("build")));
    assert!(retry(&mut guest, "../bad").is_err());
    assert_eq!(guest.state, State::default());
}

#[test]
fn selects_highest_newer_stable_release_and_commits_after_health() {
    let mut guest = Guest {
        tags: [
            "v0.1.0",
            "v0.2.0",
            "v0.10.0",
            "v1.0.0-rc.1",
            "v00.3.0",
            "main",
        ]
        .map(str::to_owned)
        .into(),
        ..Guest::default()
    };
    poll(&mut guest).unwrap();
    assert_eq!(guest.current.tag(), "v0.10.0");
    assert_eq!(guest.state, State::default());
    assert_eq!(
        guest
            .events
            .iter()
            .filter(|s| s.starts_with("build"))
            .count(),
        1
    );
    assert_eq!(guest.events.last().unwrap(), "save");
}

#[test]
fn checks_pending_tags_again_without_blocking_them() {
    let mut guest = Guest {
        ready: false,
        ..Guest::default()
    };
    poll(&mut guest).unwrap();
    assert_eq!(guest.current.tag(), "v0.1.1");
    assert!(guest.state.blocked.is_empty());
    guest.ready = true;
    poll(&mut guest).unwrap();
    assert_eq!(guest.current.tag(), "v0.2.0");
}

#[test]
fn failures_restore_previous_block_tag_and_allow_manual_retry() {
    for failure in ["build", "start", "health"] {
        let mut guest = Guest {
            failure,
            ..Guest::default()
        };
        assert!(poll(&mut guest).is_err());
        assert_eq!(guest.current.tag(), "v0.1.1");
        assert!(guest.state.pending.is_none());
        assert!(guest.state.blocked.contains("v0.2.0"));
        guest.events.clear();
        guest.failure = "";
        poll(&mut guest).unwrap();
        assert!(!guest.events.iter().any(|s| s.starts_with("build")));
        retry(&mut guest, "v0.2.0").unwrap();
        poll(&mut guest).unwrap();
        assert_eq!(guest.current.tag(), "v0.2.0");
    }
}

#[test]
fn interrupted_transaction_recovers_before_polling_and_stays_blocked() {
    let candidate = release("v0.2.0");
    let mut guest = Guest {
        current: candidate.clone(),
        state: State {
            blocked: ["v0.2.0".into()].into(),
            pending: Some(Transaction {
                previous: release("v0.1.1"),
                candidate,
            }),
        },
        ..Guest::default()
    };
    assert!(retry(&mut guest, "v0.2.0").is_err());
    poll(&mut guest).unwrap();
    assert_eq!(guest.current.tag(), "v0.1.1");
    assert!(guest.state.pending.is_none());
    assert!(guest.state.blocked.contains("v0.2.0"));
    assert!(!guest.events.iter().any(|s| s == "fetch"));
    guest.tags.push("v0.3.0".into());
    poll(&mut guest).unwrap();
    assert_eq!(guest.current.tag(), "v0.3.0");
}
