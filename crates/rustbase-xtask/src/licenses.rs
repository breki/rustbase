//! `cargo xtask licenses` -- writes the third-party attribution file
//! a project ships beside a distributed binary.
//!
//! A binary is built on a few dozen crates, nearly all permissive,
//! but permissive is not obligation-free: MIT and Apache-2.0 both
//! require the licence and copyright notice to travel with a
//! distributed binary. Shipping only the project's own `LICENSE`
//! leaves that unmet, so this command puts the third-party texts in
//! one file beside it. It covers Rust crates only: code bundled from
//! another ecosystem (an npm frontend) needs its own notice. A
//! project that only deploys to its own hosts distributes nothing; it
//! still runs the command in CI, where its failure on a crate with no
//! licence text is an early warning.
//!
//! The licence *texts* come from the crate sources already unpacked
//! in the cargo registry, so nothing is downloaded.
//!
//! **The set is what goes into building the binaries for one
//! target.** Crates reachable from one named workspace member
//! (`--package`), or else from every *distributed* one (so not
//! `xtask`'s own tree), through *normal* dependencies (so not
//! test-only crates), resolved for *one platform* (so not a
//! `uefi`-only crate). Without these restrictions the file lists
//! every package in the tree.
//!
//! **It is over-inclusive within that.** Two kinds of crate are
//! listed without being *linked*: proc-macro crates, which run at
//! compile time (`clap_derive`, `serde_derive` and their `syn` /
//! `quote` closure), and optional dependencies that only some other
//! workspace member enables -- `cargo metadata` resolves features
//! once for the whole workspace, so such an edge looks like any
//! other. Pruning either means reimplementing feature resolution.
//! So the file says "goes into building", never "linked into".
//! Separately, without `--package` a crate only one member uses is
//! listed for all of them; `--package` avoids that.
//!
//! **A crate shipping no licence text fails the command** rather
//! than being noted in passing (see `check_complete`). Raise
//! `--max-missing` with a reason when a crate genuinely has none.
//!
//! The generated header points at the project's own `LICENSE`
//! "beside this one"; copying that file into the same directory is
//! the caller's job.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::Config;
use crate::process;
use graph::{distributed_roots, reachable_normal, workspace_members};

mod graph;

/// The file's name, written in cargo's target directory unless
/// `--out` says otherwise.
const DEFAULT_OUT: &str = "THIRD-PARTY-LICENSES";

/// One dependency's attribution.
#[derive(Debug, PartialEq, Eq)]
struct Attribution {
    /// Crate name.
    name: String,
    /// Crate version.
    version: String,
    /// SPDX expression from the manifest, if it had one.
    license: Option<String>,
    /// Licence texts found beside the crate's manifest.
    texts: Vec<LicenceText>,
}

/// One licence or notice file a crate ships.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct LicenceText {
    /// Its file name, such as `LICENSE-MIT`.
    file: String,
    /// Its contents.
    body: String,
}

impl Attribution {
    /// Whether this crate shipped no licence terms.
    ///
    /// Notices (`NOTICE`, `AUTHORS`, `COPYRIGHT`) and blank files are
    /// collected but do not count as terms, so this is narrower than
    /// [`is_license_file`]. `check_complete` fails on these crates.
    #[must_use]
    fn text_missing(&self) -> bool {
        !self
            .texts
            .iter()
            .any(|t| is_license_terms(&t.file) && !t.body.trim().is_empty())
    }
}

/// File-name prefixes of a file stating licence terms.
const TERMS: &[&str] = &["license", "licence", "copying"];

/// File-name prefixes of a notice that travels with the terms.
///
/// `copyright` and `authors` are here because crates ship them and
/// they are exactly the notice MIT and Apache-2.0 require to travel:
/// `rustix` and `linux-raw-sys` explain their triple licence and the
/// LLVM exception in `COPYRIGHT`, not in `LICENSE-MIT`. `NOTICE`
/// counts: Apache-2.0 section 4(d) requires it to be carried.
const NOTICES: &[&str] = &["copyright", "authors", "notice"];

/// Whether `name` has a text extension and starts with one of
/// `prefixes`, ignoring case.
fn has_prefix(name: &str, prefixes: &[&str]) -> bool {
    let lower = name.to_ascii_lowercase();
    has_text_extension(&lower) && prefixes.iter().any(|p| lower.starts_with(p))
}

/// Whether `name` is a file stating licence *terms*, as opposed to a
/// notice accompanying them. See [`Attribution::text_missing`].
#[must_use]
fn is_license_terms(name: &str) -> bool {
    has_prefix(name, TERMS)
}

/// Whether `lower` has an extension a licence text uses: none (the
/// common bare `LICENSE`), `.txt`, `.md` or `.html`.
///
/// An allow-list, because `license.rs`, `LICENSE.spdx` or a `.toml`
/// is not a text to reproduce. A crate whose only licence file has
/// another extension is not lost silently: it fails the
/// missing-text check.
fn has_text_extension(lower: &str) -> bool {
    let ext = Path::new(lower).extension().and_then(|e| e.to_str());
    matches!(ext, None | Some("txt" | "md" | "html"))
}

/// Whether `name` is a licence or notice file: terms or a notice.
#[must_use]
fn is_license_file(name: &str) -> bool {
    has_prefix(name, TERMS) || has_prefix(name, NOTICES)
}

/// Collects the licence texts sitting beside a crate's manifest.
fn texts_beside(manifest: &Path) -> Vec<LicenceText> {
    let Some(dir) = manifest.parent() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<LicenceText> = entries
        .flatten()
        // `path().is_file()`, not `e.file_type()`, so a licence file
        // that is a symlink to a shared one (common in vendored and
        // workspace-published trees) is followed.
        .filter(|e| e.path().is_file())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if !is_license_file(&name) {
                return None;
            }
            // Lossy on purpose: a licence file is text, and a stray
            // byte is no reason to omit an attribution.
            let body = std::fs::read(e.path()).ok()?;
            Some(LicenceText {
                file: name,
                body: String::from_utf8_lossy(&body).into_owned(),
            })
        })
        .collect();
    // Deterministic, so regenerating produces no diff.
    found.sort();
    found
}

/// Reads `cargo metadata` and returns one entry per third-party
/// crate, sorted by name then version.
///
/// # Errors
///
/// Returns a message when `cargo metadata` cannot be run or parsed.
fn collect(
    cfg: &Config,
    target: &str,
    package: Option<&str>,
) -> Result<(Vec<Attribution>, PathBuf), String> {
    let out = process::cargo(
        &cfg.root,
        // No `--all-features`: default features, as a plain build
        // uses them.
        &[
            "metadata",
            "--format-version",
            "1",
            // The locked set the binary is built from; never rewrite
            // `Cargo.lock`.
            "--locked",
            // One platform, matching the archive being packaged; it
            // drops crates for other targets, such as `uefi`-only ones.
            "--filter-platform",
            target,
        ],
    )?;
    if !out.ok {
        return Err(format!("cargo metadata failed: {}", out.stderr.trim()));
    }
    let json: Value = serde_json::from_str(&out.stdout)
        .map_err(|e| format!("cargo metadata produced invalid JSON: {e}"))?;
    let target_dir =
        PathBuf::from(json["target_directory"].as_str().unwrap_or("target"));
    let crates = select(&json, package)?
        .into_iter()
        .map(|s| Attribution {
            texts: texts_beside(&s.manifest),
            name: s.name,
            version: s.version,
            license: s.license,
        })
        .collect();
    Ok((crates, target_dir))
}

/// A crate [`select`] keeps, before its texts are read.
#[derive(Debug, PartialEq, Eq)]
struct Selected {
    name: String,
    version: String,
    /// SPDX expression from the manifest, if it had one.
    license: Option<String>,
    /// Its `Cargo.toml`, beside which its licence files sit.
    manifest: PathBuf,
}

/// The pure half of [`collect`]: metadata JSON to the crates that go
/// into building the distributed binary, read from no disk.
///
/// Keeps the crates reachable through normal dependencies from the
/// workspace member called `package`, or from every member not
/// marked `publish = false`,
/// minus the members themselves, whose code the project's own
/// `LICENSE` covers. Sorted by name and version, so regenerating
/// produces no diff.
///
/// # Errors
///
/// When no workspace member is called `package`.
fn select(
    json: &Value,
    package: Option<&str>,
) -> Result<Vec<Selected>, String> {
    let members = workspace_members(json);
    let roots = match package {
        None => distributed_roots(json),
        Some(name) => vec![member_named(json, &members, name)?],
    };
    let keep = reachable_normal(json, &roots);

    // Keyed by the package **id**, not by (name, version): a
    // `[patch]` or path replacement reports the same name and version
    // as the crate it replaces, and its licence may differ. The id is
    // unique per package.
    //
    // Sorted by (name, version) for output, so regenerating produces
    // no diff, with the id breaking ties.
    let mut by_key: BTreeMap<(String, String, String), Selected> =
        BTreeMap::new();
    for pkg in json["packages"].as_array().into_iter().flatten() {
        let id = pkg["id"].as_str().unwrap_or_default();
        if members.contains(&id) || !keep.contains(id) {
            continue;
        }
        let name = pkg["name"].as_str().unwrap_or_default().to_owned();
        let version = pkg["version"].as_str().unwrap_or_default().to_owned();
        let manifest = pkg["manifest_path"].as_str().unwrap_or_default();
        by_key.insert(
            (name.clone(), version.clone(), id.to_owned()),
            Selected {
                name,
                version,
                license: pkg["license"].as_str().map(str::to_owned),
                manifest: PathBuf::from(manifest),
            },
        );
    }
    Ok(by_key.into_values().collect())
}

/// The id of the workspace member called `name`.
fn member_named<'a>(
    json: &'a Value,
    members: &[&str],
    name: &str,
) -> Result<&'a str, String> {
    let found: Vec<(&str, &str)> = json["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| Some((p["id"].as_str()?, p["name"].as_str()?)))
        .filter(|(id, _)| members.contains(id))
        .collect();
    found
        .iter()
        .find(|(_, n)| *n == name)
        .map(|(id, _)| *id)
        .ok_or_else(|| {
            let names: Vec<&str> = found.iter().map(|(_, n)| *n).collect();
            format!(
                "no workspace member is called {name:?}; the members are {}",
                names.join(", ")
            )
        })
}

/// Who the file speaks for: the one package it was built for, or the
/// project and every binary it distributes.
#[derive(Debug, PartialEq, Eq)]
struct Scope {
    /// The name the header says is distributed under `LICENSE`.
    subject: String,
    /// What the crates go into building, as the header says it.
    what: String,
}

impl Scope {
    fn new(project: &str, package: Option<&str>) -> Self {
        match package {
            Some(p) => Self {
                subject: p.to_owned(),
                what: format!("the {p} binary"),
            },
            None => Self {
                subject: project.to_owned(),
                what: format!("the binaries {project} distributes"),
            },
        }
    }
}

/// Placeholder for a crate whose manifest stated no licence.
const NO_SPDX: &str = "no SPDX expression";

/// Renders the attribution file for `scope`.
#[must_use]
fn render(scope: &Scope, crates: &[Attribution]) -> String {
    let Scope { subject, what } = scope;
    let rule = "=".repeat(70);
    let mut parts: Vec<String> = vec![
        "THIRD-PARTY LICENCES\n\n".to_owned(),
        // "go into building", not "linked into"; see the module docs.
        format!(
            "{subject} is distributed under the licence in the LICENSE file\n\
         beside this one. The Rust crates listed below go into building\n\
         {what} -- their normal dependencies, for this platform,\n\
         excluding anything used only to test them. Some run at compile\n\
         time rather than shipping inside a binary; they are listed\n\
         anyway, because an unnecessary attribution costs nothing and a\n\
         missing one is the failure that matters. Their licences and\n\
         notices follow, as those licences require. Code bundled from\n\
         another ecosystem (an npm frontend, say) needs its own notice.\n\n\
         Generated by `cargo xtask licenses` -- do not edit by hand.\n\n"
        ),
        format!("{} crates go into building {what}.\n", crates.len()),
    ];

    let missing = missing_texts(crates);
    if !missing.is_empty() {
        // Named rather than dropped: a reader must be able to see
        // the file is short of a text instead of assuming it whole.
        parts.push(format!(
            "{} of them ship no licence terms. Their SPDX expression is\n\
             recorded here and the text has to come from the crate's\n\
             own repository:\n",
            missing.len()
        ));
        parts.extend(missing.iter().map(|c| {
            format!(
                "  {} {} -- {}\n",
                c.name,
                c.version,
                c.license.as_deref().unwrap_or(NO_SPDX)
            )
        }));
    }

    for c in crates {
        parts.push(format!(
            "\n{rule}\n{} {}\nSPDX: {}\n{rule}\n",
            c.name,
            c.version,
            c.license.as_deref().unwrap_or(NO_SPDX)
        ));
        parts.extend(
            c.texts.iter().map(|t| {
                format!("\n--- {} ---\n{}\n", t.file, t.body.trim_end())
            }),
        );
    }
    parts.concat()
}

/// What [`write()`] builds the file for and where it puts it.
///
/// Not `#[non_exhaustive]`, unlike `Config`, so callers
/// can write `Options { x, ..Options::default() }`. The cost: adding a
/// field is a breaking change.
#[derive(Debug, Default)]
pub struct Options<'a> {
    /// The file to write, or a directory to write `THIRD-PARTY-LICENSES`
    /// in (an existing one, or a path ending in a separator, which is
    /// created); `None` writes it in cargo's target directory.
    pub out: Option<&'a Path>,
    /// The target triple the binary is built for; `None` is the host's.
    pub target: Option<&'a str>,
    /// The workspace member to attribute; `None` attributes every
    /// member not marked `publish = false`.
    pub package: Option<&'a str>,
    /// How many crates may ship no licence text.
    pub max_missing: usize,
}

/// Writes the attribution file as `opts` says; on success, the line
/// to print.
///
/// # Errors
///
/// Returns a message when metadata cannot be read, when no member is
/// called `opts.package`, when more crates than allowed ship no
/// licence text, or when the file cannot be written.
pub fn write(cfg: &Config, opts: &Options<'_>) -> Result<String, String> {
    let target = match opts.target {
        Some(t) => t.to_owned(),
        None => host_triple(cfg)?,
    };
    let (crates, target_dir) = collect(cfg, &target, opts.package)?;

    // A named package with no dependencies has nothing to attribute,
    // and that is right; the empty-set refusal is for a whole-project
    // run, where it means metadata went wrong.
    if opts.package.is_none() || !crates.is_empty() {
        check_complete(&crates, opts.max_missing)?;
    }

    let body = render(&Scope::new(&cfg.name, opts.package), &crates);
    // In cargo's target directory by default: a generated file, not one
    // to commit.
    let path = match opts.out {
        Some(dir) if dir.is_dir() || names_a_directory(dir) => {
            dir.join(DEFAULT_OUT)
        }
        Some(file) => file.to_path_buf(),
        None => target_dir.join(DEFAULT_OUT),
    };
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("creating {}: {e}", parent.display()))?;
    }
    std::fs::write(&path, body)
        .map_err(|e| format!("writing {}: {e}", path.display()))?;
    Ok(format!(
        "Licenses OK ({} crates for {target}) -> {}",
        crates.len(),
        path.display()
    ))
}

/// Whether `path` ends in a separator, naming a directory that need
/// not exist yet.
fn names_a_directory(path: &Path) -> bool {
    path.as_os_str()
        .to_string_lossy()
        .ends_with(['/', std::path::MAIN_SEPARATOR])
}

/// Refuses an attribution set that is missing licence texts.
///
/// Fails when more than `max_missing` crates lack licence terms.
/// Absent registry sources (a vendored build, a different
/// `CARGO_HOME`) make every crate look like that, which must fail
/// rather than write a complete-looking file.
///
/// An empty set is refused separately, because zero missing out of
/// zero crates satisfies any threshold.
///
/// # Errors
///
/// Returns a message naming the crates when more than `max_missing`
/// of them carry no text.
fn check_complete(
    crates: &[Attribution],
    max_missing: usize,
) -> Result<(), String> {
    if crates.is_empty() {
        return Err("no crates found: cargo metadata returned no \
                    reachable dependencies. Either the `resolve` \
                    graph is absent (`--no-deps` produces that) or \
                    no workspace member is publishable, and neither \
                    can be right here"
            .to_owned());
    }
    let missing = missing_texts(crates);
    if missing.len() <= max_missing {
        return Ok(());
    }
    let names: Vec<String> = missing
        .iter()
        .map(|c| format!("{} {}", c.name, c.version))
        .collect();
    Err(format!(
        "{} of {} crates ship no licence text, more than the {} \
         allowed: {}.\nIf that is correct, raise --max-missing and say \
         why. If it is every crate, the registry sources are missing \
         rather than the licences.",
        missing.len(),
        crates.len(),
        max_missing,
        names.join(", ")
    ))
}

/// The crates carrying no licence text: the one definition of a
/// missing text.
fn missing_texts(crates: &[Attribution]) -> Vec<&Attribution> {
    crates.iter().filter(|c| c.text_missing()).collect()
}

/// The target triple this build is running on.
///
/// Read from `rustc -vV` rather than assembled from
/// `std::env::consts`, which cannot distinguish `-msvc` from `-gnu`
/// and so would hand `--filter-platform` a triple rustc does not
/// know.
///
/// It fails rather than guessing a default. A guess here is not a
/// degraded result but a wrong one: a Windows archive would carry
/// the crates a Linux binary links, and the command would exit 0
/// saying so.
fn host_triple(cfg: &Config) -> Result<String, String> {
    let out = process::tool(&cfg.root, "rustc", &["-vV".as_ref()])?;
    if !out.ok {
        return Err(format!(
            "rustc -vV failed ({}): {}",
            out.status,
            out.stderr.trim()
        ));
    }
    host_from_vv(&out.stdout).map(str::to_owned).ok_or_else(|| {
        "rustc -vV printed no `host:` line, so the target is \
         unknown; pass --target explicitly"
            .to_owned()
    })
}

/// Picks the `host:` line out of `rustc -vV` output.
///
/// Split from [`host_triple`] so the parsing, the part that can be
/// wrong, is tested without spawning `rustc`.
fn host_from_vv(text: &str) -> Option<&str> {
    text.lines().find_map(|l| l.strip_prefix("host: "))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::test_repo;

    fn text(file: &str, body: &str) -> LicenceText {
        LicenceText {
            file: file.to_owned(),
            body: body.to_owned(),
        }
    }

    fn demo(texts: Vec<LicenceText>) -> Attribution {
        Attribution {
            name: "demo".to_owned(),
            version: "1.0.0".to_owned(),
            license: Some("MIT".to_owned()),
            texts,
        }
    }

    #[test]
    fn recognises_licence_and_notice_file_names() {
        // COPYRIGHT and AUTHORS are not hypothetical: `rustix` and
        // `linux-raw-sys` explain their triple licence in COPYRIGHT,
        // and `r-efi` ships AUTHORS.
        for name in [
            "LICENSE",
            "LICENSE-MIT",
            "license.txt",
            "LICENCE",
            "COPYING",
            "NOTICE",
            "notice.md",
            "COPYRIGHT",
            "AUTHORS",
            "NOTICE.html",
        ] {
            assert!(is_license_file(name), "{name} must be recognised");
        }
    }

    #[test]
    fn the_extension_rule_is_an_allow_list_and_look_alikes_are_refused() {
        // A deny-list of `rs`/`spdx` would silently admit the next
        // format nobody has invented; names merely containing the
        // word are not licences either.
        for name in [
            "license.rs",
            "LICENSE.spdx",
            "LICENSE.toml",
            "LICENSE.json",
            "LICENSE.bk",
            "src.rs",
            "README.md",
            "Cargo.toml",
            "relicense-notes.md",
        ] {
            assert!(!is_license_file(name), "{name} must be rejected");
        }
    }

    #[test]
    fn only_terms_files_state_terms() {
        for name in ["LICENSE", "LICENCE.md", "COPYING", "license.txt"] {
            assert!(is_license_terms(name), "{name} states terms");
        }
        for name in ["NOTICE", "AUTHORS", "COPYRIGHT.md"] {
            assert!(!is_license_terms(name), "{name} is a notice");
        }
    }

    #[test]
    fn only_non_empty_terms_satisfy_the_gate() {
        // A notice alone states no terms, and neither does a blank
        // LICENSE: either would leave an attribution block with no
        // licence in it, the file the gate exists to prevent.
        assert!(demo(vec![text("AUTHORS", "Someone")]).text_missing());
        assert!(demo(vec![text("LICENSE", "  \n")]).text_missing());
        let both = demo(vec![
            text("AUTHORS", "Someone"),
            text("LICENSE-MIT", "Permission is..."),
        ]);
        assert!(!both.text_missing());
    }

    #[test]
    fn texts_beside_reads_the_licence_and_notice_files_sorted() {
        let dir = test_repo::empty_dir();
        let manifest = dir.0.join("Cargo.toml");
        fs::write(&manifest, "").unwrap();
        fs::write(dir.0.join("LICENSE-MIT"), "mit terms").unwrap();
        fs::write(dir.0.join("AUTHORS"), "someone").unwrap();
        fs::write(dir.0.join("license.rs"), "fn f() {}").unwrap();
        fs::write(dir.0.join("README.md"), "readme").unwrap();
        // A non-UTF-8 byte is no reason to omit an attribution.
        fs::write(dir.0.join("NOTICE"), [b'n', 0xff]).unwrap();
        let got = texts_beside(&manifest);
        let files: Vec<&str> = got.iter().map(|t| t.file.as_str()).collect();
        assert_eq!(files, ["AUTHORS", "LICENSE-MIT", "NOTICE"]);
        assert_eq!(got[1].body, "mit terms");
        assert!(got[2].body.starts_with('n'), "{:?}", got[2].body);
        assert_eq!(texts_beside(&dir.0.join("gone/Cargo.toml")), []);
    }

    #[cfg(unix)]
    #[test]
    fn texts_beside_follows_a_symlinked_licence() {
        // Common in workspace-published trees; dropping it would list
        // the crate as shipping no licence.
        let dir = test_repo::empty_dir();
        fs::write(dir.0.join("shared"), "apache terms").unwrap();
        std::os::unix::fs::symlink(
            dir.0.join("shared"),
            dir.0.join("LICENSE-APACHE"),
        )
        .unwrap();
        let got = texts_beside(&dir.0.join("Cargo.toml"));
        assert_eq!(got, [text("LICENSE-APACHE", "apache terms")]);
    }

    /// A metadata fixture with the shapes that matter.
    ///
    /// Two workspace members: `app` (distributed) and `xtask`
    /// (`publish = false`). Five other crates, one per case:
    /// `serde`, a normal dep of `app`; `anyhow`, reached transitively
    /// through `serde`; `serde_json`, both a dev and a normal dep;
    /// `difflib`, dev-only; `clap`, reachable only from `xtask`.
    fn metadata() -> Value {
        serde_json::json!({
            "workspace_members": [
                "path+file:///repo#app@0.3.0",
                "path+file:///repo/xtask#xtask@0.1.0"
            ],
            "packages": [
                {
                    "id": "path+file:///repo#app@0.3.0",
                    "name": "app",
                    "version": "0.3.0",
                    "license": "MIT",
                    "publish": null,
                    "manifest_path": "/repo/Cargo.toml"
                },
                {
                    "id": "path+file:///repo/xtask#xtask@0.1.0",
                    "name": "xtask",
                    "version": "0.1.0",
                    "license": "MIT",
                    "publish": [],
                    "manifest_path": "/repo/xtask/Cargo.toml"
                },
                {
                    "id": "registry+x#serde@1.0.0",
                    "name": "serde",
                    "version": "1.0.0",
                    "license": "MIT OR Apache-2.0",
                    "manifest_path": "/nonexistent/serde/Cargo.toml"
                },
                {
                    "id": "registry+x#anyhow@1.0.0",
                    "name": "anyhow",
                    "version": "1.0.0",
                    "manifest_path": "/nonexistent/anyhow/Cargo.toml"
                },
                {
                    "id": "registry+x#difflib@0.4.0",
                    "name": "difflib",
                    "version": "0.4.0",
                    "license": "MIT",
                    "manifest_path": "/nonexistent/difflib/Cargo.toml"
                },
                {
                    "id": "registry+x#clap@4.0.0",
                    "name": "clap",
                    "version": "4.0.0",
                    "license": "MIT",
                    "manifest_path": "/nonexistent/clap/Cargo.toml"
                },
                {
                    "id": "registry+x#serde_json@1.0.0",
                    "name": "serde_json",
                    "version": "1.0.0",
                    "license": "MIT",
                    "manifest_path": "/nonexistent/serde_json/Cargo.toml"
                }
            ],
            "resolve": {
                "nodes": [
                    {
                        "id": "path+file:///repo#app@0.3.0",
                        "deps": [
                            {
                                "pkg": "registry+x#serde@1.0.0",
                                "dep_kinds": [{"kind": null}]
                            },
                            {
                                "pkg": "registry+x#difflib@0.4.0",
                                "dep_kinds": [{"kind": "dev"}]
                            },
                            {
                                "pkg": "registry+x#serde_json@1.0.0",
                                "dep_kinds": [
                                    {"kind": "dev"},
                                    {"kind": null}
                                ]
                            }
                        ]
                    },
                    {
                        "id": "path+file:///repo/xtask#xtask@0.1.0",
                        "deps": [
                            {
                                "pkg": "registry+x#clap@4.0.0",
                                "dep_kinds": [{"kind": null}]
                            }
                        ]
                    },
                    {
                        "id": "registry+x#serde@1.0.0",
                        "deps": [
                            {
                                "pkg": "registry+x#anyhow@1.0.0",
                                "dep_kinds": [{"kind": null}]
                            }
                        ]
                    }
                ]
            }
        })
    }

    /// The crate names [`select`] keeps from [`metadata`].
    fn kept(json: &Value) -> Vec<String> {
        select(json, None)
            .unwrap()
            .into_iter()
            .map(|c| c.name)
            .collect()
    }

    #[test]
    fn select_keeps_what_goes_into_building_the_binary_sorted() {
        // `app` is the project's own code, covered by its LICENSE; `xtask` is
        // `publish = false` and goes to nobody; `clap` is reachable
        // only through it; `difflib` arrives only on a dev edge.
        // `anyhow` arrives transitively through `serde`, and
        // `serde_json` is both dev and normal (`any(kind: null)`), so
        // both stay. Sorted, so regenerating is a no-op.
        assert_eq!(kept(&metadata()), ["anyhow", "serde", "serde_json"]);
        let anyhow = select(&metadata(), None)
            .unwrap()
            .into_iter()
            .find(|c| c.name == "anyhow")
            .unwrap();
        assert_eq!(anyhow.license, None, "no SPDX is kept, as unknown");
        assert_eq!(
            anyhow.manifest,
            Path::new("/nonexistent/anyhow/Cargo.toml")
        );
    }

    #[test]
    fn a_named_package_roots_the_walk_alone() {
        // Even an unpublished member, when asked for by name.
        let only = select(&metadata(), Some("xtask")).unwrap();
        let names: Vec<&str> = only.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["clap"]);
        let e = select(&metadata(), Some("nope")).unwrap_err();
        assert!(e.contains("\"nope\"") && e.contains("app, xtask"), "{e}");
    }

    #[test]
    fn the_header_says_what_the_file_covers() {
        let all = render(&Scope::new("proj", None), &[]);
        assert!(all.contains("proj is distributed under"), "{all}");
        assert!(all.contains("the binaries proj distributes"), "{all}");
        assert!(all.contains("Rust crates"), "{all}");
        assert!(all.contains("npm frontend"), "{all}");
        let one = render(&Scope::new("proj", Some("cli")), &[]);
        assert!(one.contains("cli is distributed under"), "{one}");
        assert!(one.contains("0 crates go into building the cli binary"));
    }

    #[test]
    fn an_absent_resolve_graph_keeps_nothing() {
        // `--no-deps` emits no `resolve`. The walk must not fall back
        // to every package; the empty set is what `check_complete`
        // refuses.
        let mut json = metadata();
        json.as_object_mut().unwrap().remove("resolve");
        assert_eq!(kept(&json), Vec::<String>::new());
    }

    #[test]
    fn a_named_registry_still_counts_as_distributed() {
        // `publish` has three states: `null`, `[]` (`publish =
        // false`) and a list naming registries, which is distributed.
        let mut json = metadata();
        json["packages"][0]["publish"] =
            serde_json::json!(["internal-registry"]);
        assert!(kept(&json).contains(&"serde".to_owned()));
    }

    #[test]
    fn keeps_both_of_two_packages_sharing_name_and_version() {
        // A `[patch]` or path replacement reports the same name and
        // version as the crate it replaces; both are kept, since the
        // fork's licence may differ.
        let json = serde_json::json!({
            "workspace_members": ["path+file:///repo#app@0.3.0"],
            "packages": [
                {
                    "id": "path+file:///repo#app@0.3.0",
                    "name": "app",
                    "version": "0.3.0",
                    "publish": null,
                    "manifest_path": "/repo/Cargo.toml"
                },
                {
                    "id": "registry+crates.io#serde@1.0.0",
                    "name": "serde",
                    "version": "1.0.0",
                    "license": "MIT OR Apache-2.0",
                    "manifest_path": "/nonexistent/a/Cargo.toml"
                },
                {
                    "id": "path+file:///forks/serde#serde@1.0.0",
                    "name": "serde",
                    "version": "1.0.0",
                    "license": "GPL-3.0-only",
                    "manifest_path": "/nonexistent/b/Cargo.toml"
                }
            ],
            // Both reachable as normal deps, so the walk keeps both
            // and the (name, version) collision is the only thing
            // that could drop one.
            "resolve": {
                "nodes": [{
                    "id": "path+file:///repo#app@0.3.0",
                    "deps": [
                        {
                            "pkg": "registry+crates.io#serde@1.0.0",
                            "dep_kinds": [{"kind": null}]
                        },
                        {
                            "pkg": "path+file:///forks/serde#serde@1.0.0",
                            "dep_kinds": [{"kind": null}]
                        }
                    ]
                }]
            }
        });
        let got = select(&json, None).unwrap();
        assert_eq!(got.len(), 2, "both packages must survive");
        let licences: Vec<Option<&str>> =
            got.iter().map(|c| c.license.as_deref()).collect();
        assert!(licences.contains(&Some("GPL-3.0-only")), "{licences:?}");
    }

    #[test]
    fn reads_the_host_triple_out_of_rustc_vv() {
        let vv = "rustc 1.90.0 (abc 2026-01-01)\n\
                  binary: rustc\n\
                  host: x86_64-pc-windows-msvc\n\
                  release: 1.90.0\n";
        assert_eq!(host_from_vv(vv), Some("x86_64-pc-windows-msvc"));
        // Not a default: a guessed triple resolves another platform.
        assert_eq!(host_from_vv("rustc 1.90.0\nbinary: rustc\n"), None);
    }

    /// An attribution with or without a bundled text.
    fn attribution(name: &str, with_text: bool) -> Attribution {
        Attribution {
            name: name.to_owned(),
            texts: if with_text {
                vec![text("LICENSE", "text")]
            } else {
                Vec::new()
            },
            ..demo(Vec::new())
        }
    }

    #[test]
    fn check_complete_counts_the_missing_texts() {
        let ok = vec![attribution("a", true), attribution("b", true)];
        assert_eq!(check_complete(&ok, 0), Ok(()));
        let one = vec![attribution("a", true), attribution("b", false)];
        let err = check_complete(&one, 0).unwrap_err();
        assert!(err.contains("b 1.0.0") && err.contains("--max-missing"));
        let two = vec![attribution("a", false), attribution("b", false)];
        assert_eq!(check_complete(&two, 2), Ok(()));
        assert!(check_complete(&two, 1).is_err());
    }

    #[test]
    fn every_crate_text_less_is_the_registry_not_the_licences() {
        // Absent registry sources make every crate look unlicensed.
        let crates: Vec<Attribution> = (0..50)
            .map(|i| attribution(&format!("c{i}"), false))
            .collect();
        let err = check_complete(&crates, 0).unwrap_err();
        assert!(err.contains("50 of 50"), "{err}");
        assert!(err.contains("registry sources are missing"), "{err}");
        // Zero missing out of zero satisfies any threshold, so the
        // emptiness is its own check.
        let err = check_complete(&[], 99).unwrap_err();
        assert!(err.contains("no crates found"), "{err}");
    }

    #[test]
    fn renders_every_crate_with_its_text_and_names_the_missing() {
        let mut full =
            demo(vec![text("LICENSE-MIT", "Permission is hereby granted")]);
        full.version = "2.0.0".to_owned();
        let body = render(&Scope::new("app", None), &[full]);
        assert!(body.contains("app is distributed under"), "{body}");
        assert!(body.contains("demo 2.0.0\nSPDX: MIT"), "{body}");
        assert!(body.contains("--- LICENSE-MIT ---"), "{body}");
        assert!(body.contains("Permission is hereby granted"), "{body}");
        assert!(!body.contains("ship no licence terms"), "{body}");
        let mut bare = attribution("bare", false);
        bare.license = None;
        let body = render(&Scope::new("app", None), &[bare]);
        assert!(body.contains("ship no licence terms"), "{body}");
        assert!(body.contains("bare 1.0.0 -- no SPDX expression"), "{body}");
        let body = render(&Scope::new("app", None), &[]);
        assert!(body.contains("0 crates go into building"), "{body}");
    }

    #[test]
    fn write_attributes_this_workspace_into_target_by_default() {
        // Shape only: the zero-missing policy is CI's step, so a new
        // crate without a licence text fails there, not here.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        // The published crate unpacks with no workspace around it.
        if !root.join("Cargo.lock").is_file() {
            eprintln!("not inside the rustbase workspace; skipping");
            return;
        }
        let cfg = Config::for_project("demo", root.clone());
        let opts = Options {
            max_missing: usize::MAX,
            ..Options::default()
        };
        let line = write(&cfg, &opts).unwrap();
        assert!(line.starts_with("Licenses OK ("), "{line}");
        // Cargo's target directory, wherever CARGO_TARGET_DIR puts it.
        let out = PathBuf::from(line.rsplit_once(" -> ").unwrap().1);
        assert!(out.ends_with(DEFAULT_OUT), "{line}");
        let body = fs::read_to_string(&out).unwrap();
        assert!(body.starts_with("THIRD-PARTY LICENCES\n"), "{body}");
        assert!(body.contains("demo is distributed under"), "{body}");
        assert!(body.contains("\n--- LICENSE"), "no licence text: {body}");
        // One member, into a directory: the file lands inside it.
        let dir = test_repo::empty_dir();
        let one = Options {
            out: Some(&dir.0),
            package: Some("rustbase"),
            max_missing: usize::MAX,
            ..Options::default()
        };
        write(&cfg, &one).unwrap();
        let body = fs::read_to_string(dir.0.join(DEFAULT_OUT)).unwrap();
        assert!(body.contains("rustbase is distributed under"), "{body}");
        assert!(body.contains("building\nthe rustbase binary"), "{body}");
    }

    #[test]
    fn a_named_package_with_no_dependencies_gets_an_empty_file() {
        // Nothing to attribute is right for a dependency-free binary;
        // the empty-set refusal is for a whole-project run.
        let dir = lone_crate();
        let cfg = Config::for_project("lone", dir.0.clone());
        let out_dir = dir.0.join("dist").join("pkg");
        let mut out_arg = out_dir.as_os_str().to_owned();
        out_arg.push(std::path::MAIN_SEPARATOR_STR);
        let opts = Options {
            out: Some(Path::new(&out_arg)),
            package: Some("lone"),
            ..Options::default()
        };
        write(&cfg, &opts).unwrap();
        // A path ending in a separator is a directory, created.
        let body = fs::read_to_string(out_dir.join(DEFAULT_OUT)).unwrap();
        assert!(body.contains("0 crates go into building the lone binary"));
    }

    /// A one-crate workspace, `lone`, with no dependencies.
    fn lone_crate() -> test_repo::TempRepo {
        let dir = test_repo::empty_dir();
        fs::create_dir_all(dir.0.join("src")).unwrap();
        fs::write(
            dir.0.join("Cargo.toml"),
            "[package]\nname = \"lone\"\nversion = \"0.1.0\"\n\
             edition = \"2024\"\n",
        )
        .unwrap();
        fs::write(dir.0.join("src/lib.rs"), "").unwrap();
        // `--locked` needs a lockfile; an empty project's is trivial.
        fs::write(
            dir.0.join("Cargo.lock"),
            "version = 4\n\n[[package]]\n\
             name = \"lone\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        dir
    }

    #[test]
    fn a_workspace_with_nothing_to_attribute_is_refused() {
        let dir = test_repo::empty_dir();
        fs::create_dir_all(dir.0.join("src")).unwrap();
        fs::write(
            dir.0.join("Cargo.toml"),
            "[package]\nname = \"lone\"\nversion = \"0.1.0\"\n\
             edition = \"2024\"\n",
        )
        .unwrap();
        fs::write(dir.0.join("src/lib.rs"), "").unwrap();
        // `--locked` needs a lockfile; an empty project's is trivial.
        fs::write(
            dir.0.join("Cargo.lock"),
            "version = 4\n\n[[package]]\n\
             name = \"lone\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        let cfg = Config::for_project("lone", dir.0.clone());
        let out = dir.0.join("out");
        let opts = Options {
            out: Some(&out),
            ..Options::default()
        };
        let e = write(&cfg, &opts).unwrap_err();
        assert!(e.contains("no crates found"), "{e}");
        assert!(!out.exists(), "a refused run wrote the file");
    }
}
