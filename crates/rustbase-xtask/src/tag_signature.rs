//! `cargo xtask verify-release-tag <tag>` -- checks that a release
//! tag carries an SSH signature from a key listed in
//! [`RELEASE_SIGNERS`].
//!
//! Opt-in: a project signs its release tags by committing that
//! file, and `/release` then creates the tag with `git tag -s` and
//! runs this check on it. Without the file nothing calls it. Each
//! signer also sets git's `gpg.format` to `ssh` and
//! `user.signingkey` to their key. A line of the file, built from the
//! signer's `.pub` file:
//!
//! ```text
//! release@example.com namespaces="git" ssh-ed25519 AAAAC3Nza...
//! ```
//!
//! The first field (ssh-keygen's "principal") is a free label,
//! usually the signer's email.
//!
//! Limit: the file is read from the working tree, so anyone who can
//! commit to the repository can add their own key. The check catches
//! mistakes (an unsigned tag, the wrong key), not tampering.

use std::ffi::{OsStr, OsString};
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
    // `show-ref --verify` accepts only this exact ref. A plain lookup
    // of `refs/tags/<tag>` falls back to other ref folders when the
    // tag is missing, finding `refs/remotes/refs/tags/<tag>`, say.
    // The later calls take the object id, so all read one object.
    let reference = format!("refs/tags/{tag}");
    let oid = git_stdout(repo, &["show-ref", "--verify", "--hash", &reference])
        .map_err(|e| format!("tag {tag} not found: {e}"))?;
    let oid = oid.trim();
    if git_stdout(repo, &["cat-file", "-t", oid])?.trim() != "tag" {
        return Err(format!(
            "{reference} is not an annotated tag; create it with \
             `git tag -s`"
        ));
    }
    check_tag_object(tag, &git_stdout(repo, &["cat-file", "tag", oid])?)?;

    // An `OsString`, so a non-UTF-8 path reaches git unchanged.
    let mut allowed = OsString::from("gpg.ssh.allowedSignersFile=");
    allowed.push(signers);
    // The SSH verifier is pinned and the OpenPGP and X.509 verifiers
    // point at programs that do not exist, so neither the developer's
    // GPG keyring nor their git config can change the result.
    let mut args: Vec<&OsStr> = vec![OsStr::new("-c"), &allowed];
    args.extend(
        [
            "-c",
            "gpg.ssh.program=ssh-keygen",
            "-c",
            "gpg.openpgp.program=rustbase-xtask-openpgp-disabled",
            "-c",
            "gpg.x509.program=rustbase-xtask-x509-disabled",
            "verify-tag",
            oid,
        ]
        .map(OsStr::new),
    );
    // A git that does not start is not a trust verdict: `?` passes it on.
    let out = process::tool(repo, "git", &args)?;
    if out.ok {
        Ok(())
    } else {
        Err(format!(
            "tag {tag} signature is not trusted by {}: {}",
            signers.display(),
            failure_text(&out)
        ))
    }
}

/// The first line of an SSH signature.
const SSH_SIGNATURE: &str = "-----BEGIN SSH SIGNATURE-----";

/// The first line of every signature git knows, SSH included. git
/// verifies the block opened by the last line starting with one of
/// these.
const SIGNATURE_STARTS: [&str; 4] = [
    SSH_SIGNATURE,
    "-----BEGIN PGP SIGNATURE-----",
    "-----BEGIN PGP MESSAGE-----",
    "-----BEGIN SIGNED MESSAGE-----",
];

/// Checks the raw tag object (`git cat-file tag` output): its `tag`
/// header must name `tag`, and the signature git will verify must be
/// an SSH one.
///
/// The signature half only makes the refusal precise ("not
/// SSH-signed", "carries no signature"); the disabled verifier
/// programs in `verify` are what stop a non-SSH tag passing.
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
    let last = object
        .lines()
        .rev()
        .find(|line| SIGNATURE_STARTS.iter().any(|s| line.starts_with(s)));
    match last {
        Some(line) if line.starts_with(SSH_SIGNATURE) => Ok(()),
        Some(_) => Err(format!(
            "tag {tag} is not SSH-signed; only SSH signatures are \
             checked against the allowed-signers file"
        )),
        None => Err(format!(
            "tag {tag} carries no signature; if `git tag -s` made it, \
             the signing step failed (check its output: a \
             passphrase-protected key needs ssh-agent)"
        )),
    }
}

/// Runs `git <args>` in `repo`: its standard output, or a message
/// naming the subcommand and its exit status when it fails.
fn git_stdout(repo: &Path, args: &[&str]) -> Result<String, String> {
    let os: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
    let out = process::tool(repo, "git", &os)?;
    if out.ok {
        Ok(out.stdout)
    } else {
        Err(format!(
            "git {} failed: {}",
            args.first().copied().unwrap_or_default(),
            failure_text(&out)
        ))
    }
}

/// A failed run's error output, or its exit status when it printed
/// none.
fn failure_text(out: &process::Captured) -> String {
    let stderr = out.stderr.trim();
    if stderr.is_empty() {
        out.status.clone()
    } else {
        format!("{stderr} ({})", out.status)
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::{
        RELEASE_SIGNERS, check_tag_object, failure_text, verify,
        verify_release_tag,
    };
    use crate::Config;
    use crate::process::Captured;
    use crate::test_repo::{TempRepo, empty_dir};

    /// git in the fixture repository `repo`, with the empty global
    /// config beside it and no system config, so the developer's own
    /// signing settings (`tag.gpgsign`, say) cannot leak in.
    fn isolated_git(repo: &Path) -> Command {
        let mut cmd = Command::new("git");
        cmd.current_dir(repo)
            .env("GIT_CONFIG_GLOBAL", repo.with_file_name("empty.gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1");
        cmd
    }

    /// Runs `git <args>` in the fixture repository, asserting success.
    fn run_git(repo: &Path, args: &[&str]) {
        let out = isolated_git(repo)
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
        std::fs::write(dir.0.join("empty.gitconfig"), "").unwrap();
        run_git(&repo, &["init", "-q"]);
        run_git(&repo, &["config", "user.name", "Test"]);
        run_git(&repo, &["config", "user.email", "release@example.com"]);
        run_git(&repo, &["commit", "-q", "--allow-empty", "-m", "init"]);
        repo
    }

    fn signed_tag(repo: &Path, tag: &str, key: &Path) {
        let key = key.to_str().unwrap();
        run_git(
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
        run_git(&repo, &["tag", "-a", "v1.0.0", "-m", "v1.0.0"]);
        let signers = signers_for(&dir, &key);
        let err = verify(&repo, "v1.0.0", &signers).unwrap_err();
        assert!(
            err.contains("carries no signature") && err.contains("ssh-agent"),
            "{err}"
        );
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
        run_git(&repo, &["tag", "v1.0.0"]);
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
        let err = verify(&repo, "v9.9.9", &signers).unwrap_err();
        assert!(
            err.starts_with("tag v9.9.9 not found: git show-ref"),
            "{err}"
        );
    }

    /// With the tag deleted, a plain lookup of `refs/tags/v1.0.0`
    /// would find the same object under
    /// `refs/remotes/refs/tags/v1.0.0`; that is not the tag.
    #[test]
    fn tag_object_outside_refs_tags_is_refused() {
        let dir = empty_dir();
        let key = keygen(&dir.0, "trusted");
        let repo = repo(&dir);
        signed_tag(&repo, "v1.0.0", &key);
        let oid = isolated_git(&repo)
            .args(["rev-parse", "refs/tags/v1.0.0"])
            .output()
            .unwrap();
        let oid = String::from_utf8(oid.stdout).unwrap();
        run_git(&repo, &["tag", "-d", "v1.0.0"]);
        run_git(
            &repo,
            &["update-ref", "refs/remotes/refs/tags/v1.0.0", oid.trim()],
        );
        let signers = signers_for(&dir, &key);
        let err = verify(&repo, "v1.0.0", &signers).unwrap_err();
        assert!(err.starts_with("tag v1.0.0 not found"), "{err}");
    }

    /// A tag object as `git cat-file tag` prints it: headers naming
    /// `name`, a blank line, then `body`.
    fn tag_object(name: &str, body: &str) -> String {
        format!(
            "object 0123\ntype commit\ntag {name}\ntagger T <t@t> 0 +0000\n\n\
             Release\n{body}"
        )
    }

    #[test]
    fn tag_object_must_name_the_tag_and_carry_only_an_ssh_signature() {
        let ssh = "-----BEGIN SSH SIGNATURE-----\nAAAA\n\
                   -----END SSH SIGNATURE-----\n";
        let pgp = "-----BEGIN PGP SIGNATURE-----\nAAAA\n";
        let x509 = "-----BEGIN SIGNED MESSAGE-----\nAAAA\n";
        assert_eq!(check_tag_object("v1", &tag_object("v1", ssh)), Ok(()));
        let cases = [
            (tag_object("v0", ssh), "named v0"),
            ("object 0123\n\nRelease\n".to_owned(), "named (none)"),
            // git verifies the last signature block, matched by prefix.
            (tag_object("v1", &format!("{ssh}{pgp}")), "not SSH-signed"),
            (
                tag_object(
                    "v1",
                    &format!("{ssh}-----BEGIN PGP MESSAGE-----\n"),
                ),
                "not SSH-signed",
            ),
            (
                tag_object(
                    "v1",
                    &format!("{ssh}{}", pgp.replace("-\n", "-x\n")),
                ),
                "not SSH-signed",
            ),
            (tag_object("v1", x509), "not SSH-signed"),
            (tag_object("v1", ""), "carries no signature"),
        ];
        for (object, expected) in cases {
            let err = check_tag_object("v1", &object).unwrap_err();
            assert!(err.contains(expected), "{expected}: {err}");
        }
    }

    #[test]
    fn a_failure_with_no_error_output_reports_its_status() {
        let out = |stderr: &str| Captured {
            ok: false,
            status: "exit status: 1".to_owned(),
            stdout: String::new(),
            stderr: stderr.to_owned(),
        };
        assert_eq!(failure_text(&out(" \n")), "exit status: 1");
        assert_eq!(failure_text(&out("bad\n")), "bad (exit status: 1)");
    }

    /// An old signed tag object placed under a newer ref name must not
    /// pass as the newer release.
    #[test]
    fn signed_tag_under_another_name_is_refused() {
        let dir = empty_dir();
        let key = keygen(&dir.0, "trusted");
        let repo = repo(&dir);
        signed_tag(&repo, "v1.0.0", &key);
        run_git(
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
    ///
    /// Not on Windows: Git for Windows' `gpg` is an MSYS program that
    /// cannot use a Windows `GNUPGHOME` path. The table test above
    /// covers the refusal everywhere.
    #[cfg(not(windows))]
    #[test]
    fn gpg_signed_tag_is_refused() {
        let dir = empty_dir();
        let key = keygen(&dir.0, "trusted");
        let repo = repo(&dir);
        if !gpg_signed_tag(&repo, "v1.0.0") {
            eprintln!("gpg not installed; skipping");
            return;
        }
        let signers = signers_for(&dir, &key);
        let err = verify(&repo, "v1.0.0", &signers).unwrap_err();
        assert!(err.contains("not SSH-signed"), "{err}");
    }

    #[cfg(not(windows))]
    #[test]
    fn the_gpg_home_leaves_room_for_the_agent_sockets() {
        use std::os::unix::fs::PermissionsExt;
        // The home of a failing macOS run (#80): its longest agent
        // socket path came to exactly the 103-byte limit.
        let macos = Path::new(concat!(
            "/var/folders/n6/hn37q6pj7sj_9dmqqc2113800000gn/T/",
            "rustbase-xtask-test-33731-47/gnupg"
        ));
        assert!(!agent_sockets_fit(macos));
        let home = gpg_home();
        assert!(agent_sockets_fit(&home.0), "{}", home.0.display());
        let mode = std::fs::metadata(&home.0).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700, "gpg warns on a shared home");
    }

    /// Signs `tag` with a throwaway `OpenPGP` key kept in a private
    /// `GNUPGHOME` (gpg's key store directory). Returns false when gpg
    /// is not installed.
    #[cfg(not(windows))]
    fn gpg_signed_tag(repo: &Path, tag: &str) -> bool {
        let guard = gpg_home();
        let home = guard.0.as_path();
        assert!(agent_sockets_fit(home), "{}", home.display());
        let Ok(generated) = Command::new("gpg")
            .env("GNUPGHOME", home)
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
        let out = isolated_git(repo)
            .env("GNUPGHOME", home)
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
            .env("GNUPGHOME", home)
            .args(["--kill", "gpg-agent"])
            .status();
        true
    }

    /// A private gpg home under `/tmp`, removed when dropped. gpg keeps
    /// its agent's sockets in the home, and a socket path is limited to
    /// 103 bytes on macOS, which the system temp folder there nearly
    /// fills (#80).
    #[cfg(not(windows))]
    fn gpg_home() -> TempRepo {
        use std::os::unix::fs::PermissionsExt;
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let home = PathBuf::from("/tmp").join(format!(
            "rbx-gpg-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&home).unwrap();
        let private = std::fs::Permissions::from_mode(0o700);
        std::fs::set_permissions(&home, private).unwrap();
        TempRepo(home)
    }

    /// Whether gpg's longest agent socket path in `home` stays clear of
    /// the 103-byte limit; the margin is there because a run at exactly
    /// 103 still failed (#80).
    #[cfg(not(windows))]
    fn agent_sockets_fit(home: &Path) -> bool {
        home.as_os_str().len() + "/S.gpg-agent.browser".len() < 100
    }
}
