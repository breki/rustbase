// Copied from silkboard's `silkboard-updater` crate at silkboard tag
// v0.3.0 (MIT, same owner); see the crate README, "Provenance".

//! Verified release selection and durable upgrade recovery.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::outcome::{Admission, Candidate, Error, Outcome, Stage};

/// A stable, canonical numeric release version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version([u64; 3]);

impl Version {
    /// Parse `vMAJOR.MINOR.PATCH`, excluding aliases and prereleases.
    #[must_use]
    pub fn parse(tag: &str) -> Option<Self> {
        let parts: Vec<_> = tag.strip_prefix('v')?.split('.').collect();
        if parts.len() != 3 {
            return None;
        }
        let mut numbers = [0; 3];
        for (index, part) in parts.iter().enumerate() {
            if part.is_empty()
                || !part.bytes().all(|b| b.is_ascii_digit())
                || (part.len() > 1 && part.starts_with('0'))
            {
                return None;
            }
            numbers[index] = part.parse().ok()?;
        }
        Some(Self(numbers))
    }
}

/// A validated stable release and its full Git commit identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "UncheckedRelease")]
pub struct Release {
    tag: String,
    commit: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UncheckedRelease {
    tag: String,
    commit: String,
}

impl TryFrom<UncheckedRelease> for Release {
    type Error = String;
    fn try_from(value: UncheckedRelease) -> Result<Self, Self::Error> {
        Self::new(&value.tag, &value.commit)
    }
}

impl Release {
    /// Validate a release identity read from an external boundary.
    ///
    /// # Errors
    /// Rejects noncanonical tags and non-SHA-1 commit identities.
    pub fn new(tag: &str, commit: &str) -> Result<Self, String> {
        if Version::parse(tag).is_none()
            || commit.len() != 40
            || !commit
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("invalid release identity".into());
        }
        Ok(Self {
            tag: tag.into(),
            commit: commit.into(),
        })
    }

    /// The canonical stable release name, not signature evidence.
    #[must_use]
    pub fn tag(&self) -> &str {
        &self.tag
    }

    /// The exact full commit identifier.
    #[must_use]
    pub fn commit(&self) -> &str {
        &self.commit
    }

    /// Validate a signed tag's direct commit target and embedded name.
    /// Signature verification is a separate mandatory backend operation.
    ///
    /// # Errors
    /// Rejects lightweight, indirect, aliased, malformed, or unsigned objects.
    pub fn from_tag(tag: &str, object: &str) -> Result<Self, String> {
        let mut lines = object.lines();
        let commit = lines
            .next()
            .and_then(|s| s.strip_prefix("object "))
            .ok_or("missing direct tag target")?;
        if lines.next() != Some("type commit")
            || lines.next() != Some(format!("tag {tag}").as_str())
            || !object.contains("\n-----BEGIN SSH SIGNATURE-----\n")
            || !object.ends_with("-----END SSH SIGNATURE-----\n")
        {
            return Err("tag must bind its name and direct commit with an SSH signature".into());
        }
        Self::new(tag, commit)
    }
}

/// An interrupted or uncommitted upgrade always restores `previous`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transaction {
    /// Previously healthy release, retained during the upgrade.
    pub previous: Release,
    /// Candidate blocked until successful completion or explicit retry.
    pub candidate: Release,
}

/// Root-owned durable updater state; pending transactions survive reboot.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    /// Failed release names remain blocked even if their refs move.
    pub blocked: BTreeSet<String>,
    /// Written and synced before any build or activation.
    pub pending: Option<Transaction>,
}

/// Deployment boundaries implemented by the installed, operator-owned updater.
pub trait Backend {
    /// Read and validate root-owned persistent state.
    ///
    /// # Errors
    /// Returns boundary failures without treating unavailable evidence as success.
    fn load(&mut self) -> Result<State, String>;
    /// Atomically write and sync persistent state before returning.
    ///
    /// # Errors
    /// Returns boundary failures without treating unavailable evidence as success.
    fn save(&mut self, state: &State) -> Result<(), String>;
    /// Read the current root-owned release identity.
    ///
    /// # Errors
    /// Returns boundary failures without treating unavailable evidence as success.
    fn current(&mut self) -> Result<Release, String>;
    /// Fetch current remote tags, removing deleted local refs.
    ///
    /// # Errors
    /// Returns boundary failures without treating unavailable evidence as success.
    fn refresh(&mut self) -> Result<Vec<String>, String>;
    /// Return only a trusted signed tag with passing exact release checks.
    /// Pending or rejected tags return None; transport failures return errors.
    ///
    /// # Errors
    /// Returns boundary failures without treating unavailable evidence as success.
    fn admit(&mut self, tag: &str) -> Result<Option<Release>, String>;
    /// Classify admission when the backend can distinguish waiting from rejection.
    /// The engine calls only this; the default derives it from `admit`
    /// for a backend that cannot tell waiting from rejected.
    ///
    /// # Errors
    /// Returns unavailable evidence or boundary errors.
    fn inspect(&mut self, tag: &str) -> Result<Admission, String> {
        self.admit(tag).map(|release| {
            release.map_or(Admission::NotAdmitted, |release| {
                Admission::Eligible { release }
            })
        })
    }
    /// Build unprivileged and snapshot regular artifacts into root-owned staging.
    ///
    /// # Errors
    /// Returns boundary failures without treating unavailable evidence as success.
    fn build(&mut self, release: &Release) -> Result<(), String>;
    /// Atomically switch current and restart the service.
    ///
    /// # Errors
    /// Returns boundary failures without treating unavailable evidence as success.
    fn activate(&mut self, release: &Release) -> Result<(), String>;
    /// Check the application's health and exact running release identity.
    ///
    /// # Errors
    /// Returns boundary failures without treating unavailable evidence as success.
    fn healthy(&mut self, release: &Release) -> Result<(), String>;
}

/// Poll once, or recover a persisted transaction before considering updates.
///
/// # Errors
/// Returns transport, persistence, build, health, or recovery failures. A failed
/// recovery retains the durable transaction for the next invocation.
pub fn poll(backend: &mut impl Backend) -> Result<Outcome, Error> {
    let mut state = boundary(Stage::State, backend.load())?;
    if state.pending.is_some() {
        let current = recover(backend, &mut state)?;
        return Ok(Outcome::Recovered {
            current,
            blocked: state.blocked,
        });
    }
    let current = boundary(Stage::Current, backend.current())?;
    boundary(Stage::Health, backend.healthy(&current))?;
    let version = Version::parse(current.tag())
        .ok_or_else(|| Error::new(Stage::Current, "invalid current version"))?;
    let mut tags: Vec<_> = boundary(Stage::Fetch, backend.refresh())?
        .into_iter()
        .filter_map(|tag| Version::parse(&tag).map(|v| (v, tag)))
        .filter(|(v, tag)| *v > version && !state.blocked.contains(tag))
        .collect();
    tags.sort_by(|a, b| b.cmp(a));
    let mut not_admitted = Vec::new();
    for (_, tag) in tags {
        let admission = boundary(Stage::Admission, backend.inspect(&tag))?;
        let Admission::Eligible { release: candidate } = admission else {
            not_admitted.push(Candidate { tag, admission });
            continue;
        };
        if candidate.tag() != tag
            || Release::new(candidate.tag(), candidate.commit()).is_err()
        {
            return Err(Error::new(
                Stage::Admission,
                "admitted identity differs from requested tag",
            ));
        }
        state.blocked.insert(tag);
        state.pending = Some(Transaction {
            previous: current.clone(),
            candidate: candidate.clone(),
        });
        boundary(Stage::State, backend.save(&state))?;
        if let Err(mut error) = upgrade(backend, &candidate) {
            error.candidate = Some(Box::new(candidate));
            match recover(backend, &mut state) {
                Ok(previous) => error.restored = Some(Box::new(previous)),
                Err(recovery) => error.recovery = Some(Box::new(recovery)),
            }
            return Err(error);
        }
        state.pending = None;
        state.blocked.remove(candidate.tag());
        boundary(Stage::State, backend.save(&state))?;
        return Ok(Outcome::Activated {
            previous: current,
            current: candidate,
        });
    }
    Ok(Outcome::Unchanged {
        current,
        not_admitted,
    })
}

fn boundary<T>(stage: Stage, result: Result<T, String>) -> Result<T, Error> {
    result.map_err(|message| Error::new(stage, message))
}

fn upgrade(
    backend: &mut impl Backend,
    candidate: &Release,
) -> Result<(), Error> {
    boundary(Stage::Build, backend.build(candidate))?;
    boundary(Stage::Fetch, backend.refresh())?;
    if boundary(Stage::Admission, backend.inspect(candidate.tag()))?.release()
        != Some(candidate)
    {
        return Err(Error::new(
            Stage::Admission,
            "candidate eligibility changed during build",
        ));
    }
    boundary(Stage::Activation, backend.activate(candidate))?;
    boundary(Stage::Health, backend.healthy(candidate))
}

fn recover(
    backend: &mut impl Backend,
    state: &mut State,
) -> Result<Release, Error> {
    let transaction = state.pending.as_ref().ok_or_else(|| {
        Error::new(Stage::Recovery, "missing recovery transaction")
    })?;
    let previous = transaction.previous.clone();
    boundary(Stage::Recovery, backend.activate(&previous))?;
    boundary(Stage::Recovery, backend.healthy(&previous))?;
    state.pending = None;
    boundary(Stage::State, backend.save(state))?;
    Ok(previous)
}

/// Remove a failed-tag block without forcing deployment or weakening admission.
///
/// # Errors
/// Rejects malformed tags, pending recovery and persistence failures.
pub fn retry(backend: &mut impl Backend, tag: &str) -> Result<(), Error> {
    if Version::parse(tag).is_none() {
        return Err(Error::new(Stage::Retry, "invalid stable tag"));
    }
    let mut state = boundary(Stage::State, backend.load())?;
    if state.pending.is_some() {
        return Err(Error::new(
            Stage::Retry,
            "run poll to complete recovery first",
        ));
    }
    state.blocked.remove(tag);
    boundary(Stage::State, backend.save(&state))
}
