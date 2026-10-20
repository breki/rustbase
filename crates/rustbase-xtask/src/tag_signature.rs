//! `cargo xtask verify-release-tag <tag>` -- checks that a release
//! tag carries an SSH signature from a key listed in
//! [`RELEASE_SIGNERS`].
//!
//! Opt-in: a project signs its release tags by committing that
//! file, and `/release` then creates the tag with `git tag -s` and
//! runs this check on it. Without the file nothing calls it.
//!
//! Limit: the file comes from the commit being checked, so the check
//! catches mistakes (an unsigned tag, the wrong key), not a commit
//! that adds its own key. A trust boundary needs a copy of the keys
//! kept outside the repository.

use std::ffi::OsStr;
use std::path::Path;

use crate::{Config, process};

/// The tracked list of keys allowed to sign release tags, relative to
/// the project root. One `<principal> namespaces="git" <key-type>
/// <key>` line per key (`ssh-keygen(1)`, "ALLOWED SIGNERS").
pub const RELEASE_SIGNERS: &str = ".github/release-signers";

/// Verifies `tag` in `cfg.root` against [`RELEASE_SIGNERS`]; on
/// success, the line to print.
///
/// # Errors
///
/// Returns the reason the tag was refused.
pub fn verify_release_tag(cfg: &Config, tag: &str) -> Result<String, String> {
    verify(&cfg.root, tag, &cfg.root.join(RELEASE_SIGNERS))?;
    Ok(format!(
        "tag {tag}: good SSH signature from a key in {RELEASE_SIGNERS}"
    ))
}

/// Verifies that `tag` in the repository at `repo` is signed by a key
/// listed in `signers`.
fn verify(repo: &Path, tag: &str, signers: &Path) -> Result<(), String> {
    // A missing file makes `git verify-tag` fail with a generic
    // message, so name it here instead.
    if !signers.is_file() {
        return Err(format!(
            "allowed-signers file {} does not exist",
            signers.display()
        ));
    }
    // `refs/tags/...` in full: a bare name could resolve to a branch.
    let reference = format!("refs/tags/{tag}");
    if git(repo, &["cat-file", "-t", &reference])?.trim() != "tag" {
        return Err(format!(
            "{reference} is not an annotated tag; create it with \
             `git tag -s`"
        ));
    }
    check_tag_object(tag, &git(repo, &["cat-file", "tag", &reference])?)?;

    let allowed = format!("gpg.ssh.allowedSignersFile={}", signers.display());
    // The SSH verifier is pinned and the OpenPGP and X.509 verifiers
    // point at programs that do not exist, so neither the developer's
    // GPG keyring nor their git config can change the result.
    git(
        repo,
        &[
            "-c",
            &allowed,
            "-c",
            "gpg.ssh.program=ssh-keygen",
            "-c",
            "gpg.openpgp.program=rustbase-xtask-openpgp-disabled",
            "-c",
            "gpg.x509.program=rustbase-xtask-x509-disabled",
            "verify-tag",
            &reference,
        ],
    )
    .map(|_| ())
    .map_err(|e| {
        format!(
            "tag {tag} signature is not trusted by {}: {e}",
            signers.display()
        )
    })
}

/// The first line of an SSH signature. git picks the verifier from
/// the first signature line it finds in the tag.
const SSH_SIGNATURE: &str = "-----BEGIN SSH SIGNATURE-----";

/// The first lines of the `OpenPGP` and X.509 signatures.
const OTHER_SIGNATURES: [&str; 2] = [
    "-----BEGIN PGP SIGNATURE-----",
    "-----BEGIN SIGNED MESSAGE-----",
];

/// Checks the raw tag object (`git cat-file tag` output): its `tag`
/// header must name `tag`, and it must carry an SSH signature and no
/// other kind.
fn check_tag_object(tag: &str, object: &str) -> Result<(), String> {
    // Header lines come first and end at the first empty line.
    let named = object
        .lines()
        .take_while(|line| !line.is_empty())
        .find_map(|line| line.strip_prefix("tag "));
    if named != Some(tag) {
        return Err(format!(
            "refs/tags/{tag} holds a tag object named {}; refusing a \
             renamed tag",
            named.unwrap_or("(none)")
        ));
    }
    if object
        .lines()
        .any(|line| OTHER_SIGNATURES.contains(&line.trim_end()))
    {
        return Err(format!(
            "tag {tag} is not SSH-signed; only SSH signatures are \
             checked against the allowed-signers file"
        ));
    }
    if !object.lines().any(|line| line.trim_end() == SSH_SIGNATURE) {
        return Err(format!(
            "tag {tag} is not signed; create it with `git tag -s`"
        ));
    }
    Ok(())
}

/// Runs git in `repo`: its standard output, or its error output as
/// the message when it fails.
fn git(repo: &Path, args: &[&str]) -> Result<String, String> {
    let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
    let out = process::tool(repo, "git", &args)?;
    if out.ok {
        Ok(out.stdout)
    } else {
        Err(out.stderr.trim().to_owned())
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::{RELEASE_SIGNERS, verify, verify_release_tag};
    use crate::Config;
    use crate::test_repo::{TempRepo, empty_dir};

    /// Runs git in `dir` with an empty global config and no system
    /// config, so the developer's own signing settings (`tag.gpgsign`,
    /// say) cannot leak into the fixture.
    fn git(dir: &Path, args: &[&str]) {
        let out = Command::new("git")
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", dir.join("empty.gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .args(args)
            .output()
            .expect("git not found on PATH");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// Creates an unencrypted ed25519 key pair in `dir`; returns the
    /// private key's path.
    fn keygen(dir: &Path, name: &str) -> PathBuf {
        let key = dir.join(name);
        let status = Command::new("ssh-keygen")
            .args(["-q", "-t", "ed25519", "-N", "", "-C", name, "-f"])
            .arg(&key)
            .status()
            .expect("ssh-keygen not found on PATH; these tests need it");
        assert!(status.success());
        key
    }

    /// Writes an allowed-signers file at `path` trusting `key` for
    /// `namespaces`.
    fn write_signers(path: &Path, key: &Path, namespaces: &str) {
        let public =
            std::fs::read_to_string(key.with_extension("pub")).unwrap();
        std::fs::write(
            path,
            format!("release@example.com namespaces=\"{namespaces}\" {public}"),
        )
        .unwrap();
    }

    /// An allowed-signers file beside the repository trusting `key`.
    fn signers_for(dir: &TempRepo, key: &Path) -> PathBuf {
        let path = dir.0.join("release-signers");
        write_signers(&path, key, "git");
        path
    }

    /// A repository with one empty commit, so a tag has a target.
    fn repo(dir: &TempRepo) -> PathBuf {
        let repo = dir.0.join("repo");
        std::fs::create_dir(&repo).unwrap();
        std::fs::write(repo.join("empty.gitconfig"), "").unwrap();
        git(&repo, &["init", "-q"]);
        git(&repo, &["config", "user.name", "Test"]);
        git(&repo, &["config", "user.email", "release@example.com"]);
        git(&repo, &["commit", "-q", "--allow-empty", "-m", "init"]);
        repo
    }

    fn signed_tag(repo: &Path, tag: &str, key: &Path) {
        let key = key.to_str().unwrap();
        git(
            repo,
            &[
                "-c",
                "gpg.format=ssh",
                "-c",
                &format!("user.signingkey={key}"),
                "tag",
                "-s",
                tag,
                "-m",
                tag,
            ],
        );
    }

    #[test]
    fn tag_signed_by_a_trusted_key_passes() {
        let dir = empty_dir();
        let key = keygen(&dir.0, "trusted");
        let repo = repo(&dir);
        signed_tag(&repo, "v1.0.0", &key);
        let signers = signers_for(&dir, &key);
        assert_eq!(verify(&repo, "v1.0.0", &signers), Ok(()));
    }

    #[test]
    fn the_command_reads_the_signers_file_in_the_project() {
        let dir = empty_dir();
        let key = keygen(&dir.0, "trusted");
        let repo = repo(&dir);
        signed_tag(&repo, "v1.0.0", &key);
        let cfg = Config::for_project("p", repo.clone());
        let err = verify_release_tag(&cfg, "v1.0.0").unwrap_err();
        assert!(err.contains("does not exist"), "{err}");

        std::fs::create_dir(repo.join(".github")).unwrap();
        write_signers(&repo.join(RELEASE_SIGNERS), &key, "git");
        let line = verify_release_tag(&cfg, "v1.0.0").unwrap();
        assert!(line.contains("good SSH signature"), "{line}");
    }

    #[test]
    fn unsigned_tag_is_refused() {
        let dir = empty_dir();
        let key = keygen(&dir.0, "trusted");
        let repo = repo(&dir);
        git(&repo, &["tag", "-a", "v1.0.0", "-m", "v1.0.0"]);
        let signers = signers_for(&dir, &key);
        let err = verify(&repo, "v1.0.0", &signers).unwrap_err();
        assert!(err.contains("not signed"), "{err}");
    }

    #[test]
    fn tag_signed_by_an_unknown_key_is_refused() {
        let dir = empty_dir();
        let trusted = keygen(&dir.0, "trusted");
        let stranger = keygen(&dir.0, "stranger");
        let repo = repo(&dir);
        signed_tag(&repo, "v1.0.0", &stranger);
        let signers = signers_for(&dir, &trusted);
        let err = verify(&repo, "v1.0.0", &signers).unwrap_err();
        assert!(err.contains("not trusted"), "{err}");
    }

    #[test]
    fn missing_signers_file_is_refused() {
        let dir = empty_dir();
        let key = keygen(&dir.0, "trusted");
        let repo = repo(&dir);
        signed_tag(&repo, "v1.0.0", &key);
        let missing = dir.0.join("no-such-file");
        let err = verify(&repo, "v1.0.0", &missing).unwrap_err();
        assert!(err.contains("no-such-file"), "{err}");
    }

    #[test]
    fn lightweight_tag_is_refused() {
        let dir = empty_dir();
        let key = keygen(&dir.0, "trusted");
        let repo = repo(&dir);
        git(&repo, &["tag", "v1.0.0"]);
        let signers = signers_for(&dir, &key);
        let err = verify(&repo, "v1.0.0", &signers).unwrap_err();
        assert!(err.contains("not an annotated tag"), "{err}");
    }

    #[test]
    fn missing_tag_is_refused() {
        let dir = empty_dir();
        let key = keygen(&dir.0, "trusted");
        let repo = repo(&dir);
        let signers = signers_for(&dir, &key);
        assert!(verify(&repo, "v9.9.9", &signers).is_err());
    }

    /// An old signed tag object placed under a newer ref name must not
    /// pass as the newer release.
    #[test]
    fn signed_tag_under_another_name_is_refused() {
        let dir = empty_dir();
        let key = keygen(&dir.0, "trusted");
        let repo = repo(&dir);
        signed_tag(&repo, "v1.0.0", &key);
        git(
            &repo,
            &["update-ref", "refs/tags/v2.0.0", "refs/tags/v1.0.0"],
        );
        let signers = signers_for(&dir, &key);
        let err = verify(&repo, "v2.0.0", &signers).unwrap_err();
        assert!(err.contains("v1.0.0"), "{err}");
    }

    #[test]
    fn key_listed_for_another_namespace_is_refused() {
        let dir = empty_dir();
        let key = keygen(&dir.0, "trusted");
        let repo = repo(&dir);
        signed_tag(&repo, "v1.0.0", &key);
        let signers = dir.0.join("release-signers");
        write_signers(&signers, &key, "file");
        let err = verify(&repo, "v1.0.0", &signers).unwrap_err();
        assert!(err.contains("not trusted"), "{err}");
    }

    /// git checks an `OpenPGP` signature against the local GPG keyring
    /// and never reads the allowed-signers file, so only SSH
    /// signatures may pass.
    #[test]
    fn gpg_signed_tag_is_refused() {
        let dir = empty_dir();
        let key = keygen(&dir.0, "trusted");
        let repo = repo(&dir);
        if !gpg_signed_tag(&dir, &repo, "v1.0.0") {
            eprintln!("gpg not installed; skipping");
            return;
        }
        let signers = signers_for(&dir, &key);
        let err = verify(&repo, "v1.0.0", &signers).unwrap_err();
        assert!(err.contains("not SSH-signed"), "{err}");
    }

    /// Signs `tag` with a throwaway `OpenPGP` key kept in a private
    /// `GNUPGHOME` (gpg's key store directory). Returns false when gpg
    /// is not installed.
    fn gpg_signed_tag(dir: &TempRepo, repo: &Path, tag: &str) -> bool {
        let home = dir.0.join("gnupg");
        std::fs::create_dir(&home).unwrap();
        let Ok(generated) = Command::new("gpg")
            .env("GNUPGHOME", &home)
            .args([
                "--batch",
                "--pinentry-mode",
                "loopback",
                "--passphrase",
                "",
                "--quick-gen-key",
                "Stranger <stranger@example.com>",
                "ed25519",
                "sign",
                "never",
            ])
            .output()
        else {
            return false;
        };
        assert!(
            generated.status.success(),
            "gpg: {}",
            String::from_utf8_lossy(&generated.stderr)
        );
        let out = Command::new("git")
            .current_dir(repo)
            .env("GIT_CONFIG_GLOBAL", repo.join("empty.gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GNUPGHOME", &home)
            .args([
                "-c",
                "gpg.format=openpgp",
                "-c",
                "user.signingkey=stranger@example.com",
                "tag",
                "-s",
                tag,
                "-m",
                tag,
            ])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git tag: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        // Stop the gpg agent that signing started for this home.
        let _ = Command::new("gpgconf")
            .env("GNUPGHOME", &home)
            .args(["--kill", "gpg-agent"])
            .status();
        true
    }
}
