//! Waiting on a child process with a time limit. Kept apart from the
//! spawn in `process.rs`, which is excluded from the coverage gate, so
//! the timeout decision is tested with a fake child.

use std::io;
use std::thread;
use std::time::{Duration, Instant};

/// The two things the wait needs from a running child.
pub(crate) trait Poll {
    /// What a finished child reports.
    type Status;
    /// The status if the child has exited, without blocking.
    fn try_wait(&mut self) -> io::Result<Option<Self::Status>>;
    /// Stop the child.
    fn kill(&mut self) -> io::Result<()>;
}

/// How a wait ended.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Waited<S> {
    /// The child exited within the limit.
    Exited(S),
    /// The limit passed and the child was killed.
    TimedOut,
}

/// Poll `child` every `poll` until it exits or `limit` has passed,
/// killing it in the second case. A `limit` too large for the clock
/// waits for the child alone.
pub(crate) fn wait_until<C: Poll>(
    child: &mut C,
    limit: Duration,
    poll: Duration,
) -> io::Result<Waited<C::Status>> {
    // A limit past the clock's range is no limit.
    let deadline = Instant::now().checked_add(limit);
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Waited::Exited(status));
        }
        let left = deadline
            .map_or(poll, |d| d.saturating_duration_since(Instant::now()));
        if left.is_zero() {
            child.kill()?;
            return Ok(Waited::TimedOut);
        }
        thread::sleep(poll.min(left));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exits after `exits_after` polls, or never when `None`.
    struct Fake {
        polls: u32,
        exits_after: Option<u32>,
        killed: bool,
        kill_fails: bool,
    }

    impl Fake {
        fn new(exits_after: Option<u32>) -> Self {
            Self {
                polls: 0,
                exits_after,
                killed: false,
                kill_fails: false,
            }
        }
    }

    impl Poll for Fake {
        type Status = u32;
        fn try_wait(&mut self) -> io::Result<Option<u32>> {
            self.polls += 1;
            Ok(self.exits_after.filter(|n| self.polls > *n).map(|_| 0))
        }
        fn kill(&mut self) -> io::Result<()> {
            self.killed = true;
            if self.kill_fails {
                Err(io::Error::other("kill refused"))
            } else {
                Ok(())
            }
        }
    }

    const POLL: Duration = Duration::from_millis(1);

    #[test]
    fn a_child_that_exits_in_time_is_not_killed() {
        let mut c = Fake::new(Some(3));
        let w = wait_until(&mut c, Duration::from_secs(5), POLL).unwrap();
        assert_eq!(w, Waited::Exited(0));
        assert!(!c.killed);
    }

    #[test]
    fn a_child_that_never_exits_is_killed_at_the_limit() {
        let mut c = Fake::new(None);
        let start = Instant::now();
        let w = wait_until(&mut c, Duration::from_millis(30), POLL).unwrap();
        assert_eq!(w, Waited::TimedOut);
        assert!(c.killed);
        assert!(start.elapsed() >= Duration::from_millis(30));
        assert!(c.polls > 1, "it polled rather than slept once");
    }

    #[test]
    fn a_child_already_done_is_not_slept_on() {
        // A zero limit still reads a finished child's status first.
        let mut c = Fake::new(Some(0));
        let w = wait_until(&mut c, Duration::ZERO, POLL).unwrap();
        assert_eq!(w, Waited::Exited(0));
    }

    #[test]
    fn a_limit_too_large_to_reach_waits_without_a_deadline() {
        // `Instant + Duration` panics past the clock's range; such a
        // limit means "no limit".
        let mut c = Fake::new(Some(3));
        let w = wait_until(&mut c, Duration::MAX, POLL).unwrap();
        assert_eq!(w, Waited::Exited(0));
        assert!(!c.killed);
    }

    #[test]
    fn a_failed_kill_is_reported() {
        let mut c = Fake::new(None);
        c.kill_fails = true;
        let e = wait_until(&mut c, Duration::ZERO, POLL).unwrap_err();
        assert!(e.to_string().contains("kill refused"), "{e}");
    }
}
