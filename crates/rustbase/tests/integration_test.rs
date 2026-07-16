// REPLACE-ME SCAFFOLDING: `cli_runs_successfully` and
// `cli_verbose_flag` below are pinned to the stub binary's
// example output ("Hello from rustbase" / "verbose mode
// enabled"). The moment you add a real subcommand, rewrite
// or delete these two together with the stub `main.rs` --
// they will otherwise fail on the changed stub behaviour.
// `cli_version_flag` / `cli_help_flag` assert only
// version/help and survive the first real command; keep
// them and model new tests on that shape.
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn cli_runs_successfully() {
    Command::cargo_bin("rustbase")
        .unwrap()
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello from rustbase"));
}

#[test]
fn cli_verbose_flag() {
    Command::cargo_bin("rustbase")
        .unwrap()
        .arg("--verbose")
        .assert()
        .success()
        .stdout(predicate::str::contains("verbose mode enabled"));
}

#[test]
fn cli_version_flag() {
    Command::cargo_bin("rustbase")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("rustbase"));
}

#[test]
fn cli_help_flag() {
    Command::cargo_bin("rustbase")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage"));
}
