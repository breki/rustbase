use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask")]
struct Cli {
    #[command(subcommand)]
    command: XCommand,
}

#[derive(Subcommand)]
enum XCommand {
    /// Run clippy (deny warnings)
    Clippy,
    /// Run all tests
    Test {
        /// Optional test filter
        filter: Option<String>,
    },
    /// Run fmt + clippy + tests + coverage + duplication
    Validate,
    /// Format code
    Fmt,
    /// Run coverage check (requires cargo-llvm-cov)
    Coverage,
    /// Run code duplication check (requires code-dupes)
    Dupes,
}

/// Minimum line coverage percentage (overall).
const COVERAGE_THRESHOLD: f64 = 90.0;

/// Per-module coverage floor.
const MODULE_COVERAGE_THRESHOLD: f64 = 85.0;

/// Maximum allowed exact duplication percentage
/// (production code only, tests excluded).
const DUPLICATION_THRESHOLD: f64 = 6.0;

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        XCommand::Clippy => run_clippy(),
        XCommand::Test { filter } => run_test(filter.as_deref()),
        XCommand::Validate => run_fmt_check()
            .and_then(|()| run_clippy())
            .and_then(|()| run_test(None))
            .and_then(|()| run_coverage())
            .and_then(|()| run_dupes()),
        XCommand::Fmt => run_fmt(),
        XCommand::Coverage => run_coverage(),
        XCommand::Dupes => run_dupes(),
    };

    if let Err(e) = result {
        eprintln!("xtask error: {e}");
        std::process::exit(1);
    }
}

fn run_clippy() -> Result<(), String> {
    run_cmd(
        &cargo_bin(),
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    )
}

fn run_test(filter: Option<&str>) -> Result<(), String> {
    let mut args = vec!["test", "--workspace"];
    if let Some(f) = filter {
        if f.is_empty() {
            return Err("test filter must not be empty".into());
        }
        args.push("--");
        args.push(f);
    }
    run_cmd(&cargo_bin(), &args)
}

fn run_fmt() -> Result<(), String> {
    run_cmd(&cargo_bin(), &["fmt", "--all"])
}

fn run_fmt_check() -> Result<(), String> {
    run_cmd(&cargo_bin(), &["fmt", "--all", "--", "--check"])
}

fn run_coverage() -> Result<(), String> {
    println!(
        "-> checking coverage \
         (threshold: {COVERAGE_THRESHOLD:.1}%)"
    );

    // Regex matches main.rs binary entry points on
    // both Unix (/) and Windows (\) paths:
    //   crates/rustbase/src/bin/rustbase/main.rs
    //   crates/rustbase-web/src/main.rs
    let main_rs_regex = r"(^|[/\\])main\.rs$";

    let output = Command::new(cargo_bin())
        .args([
            "llvm-cov",
            "--workspace",
            "--exclude",
            "xtask",
            "--ignore-filename-regex",
            main_rs_regex,
            "--json",
            "--summary-only",
        ])
        .output()
        .map_err(|e| {
            format!(
                "failed to run cargo llvm-cov: {e}. \
                 Install with: cargo install cargo-llvm-cov"
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("cargo llvm-cov failed:\n{stderr}"));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("failed to parse coverage JSON: {e}"))?;

    // Extract total line coverage percentage.
    let line_pct = json["data"][0]["totals"]["lines"]["percent"]
        .as_f64()
        .ok_or("missing lines.percent in coverage JSON")?;

    let covered = json["data"][0]["totals"]["lines"]["covered"]
        .as_u64()
        .ok_or("missing lines.covered in coverage JSON")?;
    let total = json["data"][0]["totals"]["lines"]["count"]
        .as_u64()
        .ok_or("missing lines.count in coverage JSON")?;

    println!("  lines: {covered}/{total} ({line_pct:.1}%)");

    // Per-file summary.
    let mut below_threshold = Vec::new();
    if let Some(files) = json["data"][0]["files"].as_array() {
        for file in files {
            let name = file["filename"].as_str().unwrap_or("?");
            let pct =
                file["summary"]["lines"]["percent"].as_f64().unwrap_or(0.0);
            let short = name
                .rsplit_once("src\\")
                .or_else(|| name.rsplit_once("src/"))
                .map_or(name, |(_, rest)| rest);
            let marker = if pct < MODULE_COVERAGE_THRESHOLD {
                below_threshold.push((short.to_string(), pct));
                "!"
            } else {
                " "
            };
            println!("  {marker} {short:<50} {pct:>5.1}%");
        }
    }

    if line_pct < COVERAGE_THRESHOLD {
        Err(format!(
            "coverage {line_pct:.1}% is below \
             {COVERAGE_THRESHOLD:.1}% threshold"
        ))
    } else if !below_threshold.is_empty() {
        let mut msg = String::from("modules below coverage threshold:");
        for (name, pct) in &below_threshold {
            let _ = write!(msg, "\n    {name}: {pct:.1}%");
        }
        Err(msg)
    } else {
        println!(
            "  coverage OK ({line_pct:.1}% >= \
             {COVERAGE_THRESHOLD:.1}%)"
        );
        Ok(())
    }
}

fn run_dupes() -> Result<(), String> {
    println!(
        "-> checking code duplication \
         (threshold: {DUPLICATION_THRESHOLD:.1}%)"
    );

    let src_dirs = discover_src_dirs()?;

    let threshold = format!("{DUPLICATION_THRESHOLD:.1}");
    for src_dir in &src_dirs {
        run_cmd(
            "code-dupes",
            &[
                "-p",
                src_dir,
                "--exclude-tests",
                "check",
                "--max-exact-percent",
                &threshold,
            ],
        )
        .map_err(|e| {
            // Only add install hint when the binary
            // was not found, not when it ran but
            // reported excessive duplication.
            if e.contains("failed to run") {
                format!(
                    "{e}\n  Install with: \
                     cargo install code-dupes"
                )
            } else {
                e
            }
        })?;
    }

    Ok(())
}

/// Discover `src/` directories for non-xtask workspace
/// members using `cargo metadata`.
fn discover_src_dirs() -> Result<Vec<String>, String> {
    let output = Command::new(cargo_bin())
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .map_err(|e| format!("failed to run cargo metadata: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("cargo metadata failed:\n{stderr}"));
    }

    let meta: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("failed to parse cargo metadata: {e}"))?;

    let workspace_root = meta["workspace_root"]
        .as_str()
        .ok_or("missing workspace_root in metadata")?;

    let mut src_dirs = Vec::new();
    if let Some(packages) = meta["packages"].as_array() {
        for pkg in packages {
            let name = pkg["name"].as_str().unwrap_or("");
            // Skip xtask — it's build tooling, not
            // production code.
            if name == "xtask" {
                continue;
            }
            let manifest = pkg["manifest_path"].as_str().unwrap_or("");
            // Derive src/ dir from Cargo.toml path.
            if let Some(pkg_dir) = Path::new(manifest).parent() {
                let src = pkg_dir.join("src");
                if src.is_dir() {
                    src_dirs.push(src.to_string_lossy().into_owned());
                }
            }
        }
    }

    if src_dirs.is_empty() {
        return Err(format!(
            "no src/ directories found in workspace \
             at {workspace_root}"
        ));
    }

    Ok(src_dirs)
}

/// Resolve the cargo binary path. Prefers the `CARGO`
/// env var (set by cargo when running xtask) over a
/// PATH lookup.
fn cargo_bin() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".into())
}

fn run_cmd(cmd: &str, args: &[&str]) -> Result<(), String> {
    println!("-> {cmd} {}", args.join(" "));
    let status = Command::new(cmd)
        .args(args)
        .status()
        .map_err(|e| format!("failed to run {cmd}: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        match status.code() {
            Some(code) => Err(format!("{cmd} exited with {code}")),
            None => Err(format!("{cmd} terminated by signal")),
        }
    }
}
