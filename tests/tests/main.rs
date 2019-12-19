pub mod project;

use self::project::*;
use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
use std::process::Command;

fn cargo_fuzz() -> Command {
    Command::cargo_bin("cargo-fuzz").unwrap()
}

#[test]
fn help() {
    cargo_fuzz().arg("help").assert().success();
}

#[test]
fn init() {
    let project = project("init").build();
    project.cargo_fuzz().arg("init").assert().success();
    assert!(project.fuzz_dir().is_dir());
    assert!(project.fuzz_cargo_toml().is_file());
    assert!(project.fuzz_targets_dir().is_dir());
    assert!(project.fuzz_target_path("fuzz_target_1").is_file());
    project
        .cargo_fuzz()
        .arg("run")
        .arg("fuzz_target_1")
        .arg("--")
        .arg("-runs=1")
        .assert()
        .success();
}

#[test]
fn init_with_target() {
    let project = project("init_with_target").build();
    project
        .cargo_fuzz()
        .arg("init")
        .arg("-t")
        .arg("custom_target_name")
        .assert()
        .success();
    assert!(project.fuzz_dir().is_dir());
    assert!(project.fuzz_cargo_toml().is_file());
    assert!(project.fuzz_targets_dir().is_dir());
    assert!(project.fuzz_target_path("custom_target_name").is_file());
    project
        .cargo_fuzz()
        .arg("run")
        .arg("custom_target_name")
        .arg("--")
        .arg("-runs=1")
        .assert()
        .success();
}

#[test]
fn init_twice() {
    let project = project("init_twice").build();

    // First init should succeed and make all the things.
    project.cargo_fuzz().arg("init").assert().success();
    assert!(project.fuzz_dir().is_dir());
    assert!(project.fuzz_cargo_toml().is_file());
    assert!(project.fuzz_targets_dir().is_dir());
    assert!(project.fuzz_target_path("fuzz_target_1").is_file());

    // Second init should fail.
    project
        .cargo_fuzz()
        .arg("init")
        .assert()
        .stderr(predicates::str::contains("File exists (os error 17)").and(
            predicates::str::contains(format!(
                "failed to create directory {}",
                project.fuzz_dir().display()
            )),
        ))
        .failure();
}

#[test]
fn init_finds_parent_project() {
    let project = project("init_finds_parent_project").build();
    project
        .cargo_fuzz()
        .current_dir(project.root().join("src"))
        .arg("init")
        .assert()
        .success();
    assert!(project.fuzz_dir().is_dir());
    assert!(project.fuzz_cargo_toml().is_file());
    assert!(project.fuzz_targets_dir().is_dir());
    assert!(project.fuzz_target_path("fuzz_target_1").is_file());
}

#[test]
fn add() {
    let project = project("add").with_fuzz().build();
    project
        .cargo_fuzz()
        .arg("add")
        .arg("new_fuzz_target")
        .assert()
        .success();
    assert!(project.fuzz_target_path("new_fuzz_target").is_file());
    project
        .cargo_fuzz()
        .arg("run")
        .arg("new_fuzz_target")
        .arg("--")
        .arg("-runs=1")
        .assert()
        .success();
}

#[test]
fn add_twice() {
    let project = project("add").with_fuzz().build();
    project
        .cargo_fuzz()
        .arg("add")
        .arg("new_fuzz_target")
        .assert()
        .success();
    assert!(project.fuzz_target_path("new_fuzz_target").is_file());
    project
        .cargo_fuzz()
        .arg("add")
        .arg("new_fuzz_target")
        .assert()
        .stderr(
            predicate::str::contains("could not add target")
                .and(predicate::str::contains("File exists (os error 17)")),
        )
        .failure();
}

#[test]
fn list() {
    let project = project("add").with_fuzz().build();

    // Create some targets.
    project.cargo_fuzz().arg("add").arg("c").assert().success();
    project.cargo_fuzz().arg("add").arg("b").assert().success();
    project.cargo_fuzz().arg("add").arg("a").assert().success();

    // Make sure that we can list our targets, and that they're always sorted.
    project
        .cargo_fuzz()
        .arg("list")
        .assert()
        .stdout("a\nb\nc\n")
        .success();
}

#[test]
fn run_no_crash() {
    let project = project("run_no_crash")
        .with_fuzz()
        .fuzz_target(
            "no_crash",
            r#"
                #![no_main]
                use libfuzzer_sys::fuzz_target;

                fuzz_target!(|data: &[u8]| {
                    run_no_crash::pass_fuzzing(data);
                });
            "#,
        )
        .build();

    project
        .cargo_fuzz()
        .arg("run")
        .arg("no_crash")
        .arg("--")
        .arg("-runs=1000")
        .assert()
        .stderr(predicate::str::contains("Done 1000 runs"))
        .success();
}

#[test]
fn run_with_crash() {
    let project = project("run_with_crash")
        .with_fuzz()
        .fuzz_target(
            "yes_crash",
            r#"
                #![no_main]
                use libfuzzer_sys::fuzz_target;

                fuzz_target!(|data: &[u8]| {
                    run_with_crash::fail_fuzzing(data);
                });
            "#,
        )
        .build();

    project
        .cargo_fuzz()
        .arg("run")
        .arg("yes_crash")
        .arg("--")
        .arg("-runs=1000")
        .env("RUST_BACKTRACE", "1")
        .assert()
        .stderr(
            predicate::str::contains("panicked at 'I'm afraid of number 7'")
                .and(predicate::str::contains("ERROR: libFuzzer: deadly signal"))
                .and(predicate::str::contains("run_with_crash::fail_fuzzing"))
                .and(predicate::str::contains("fuzz/artifacts/yes_crash/crash-")),
        )
        .failure();
}

#[test]
fn cmin() {
    let corpus = Path::new("fuzz").join("corpus").join("foo");
    let project = project("cmin")
        .with_fuzz()
        .fuzz_target(
            "foo",
            r#"
                #![no_main]
                use libfuzzer_sys::fuzz_target;

                fuzz_target!(|data: &[u8]| {
                    let _ = data;
                });
            "#,
        )
        .file(corpus.join("0"), "")
        .file(corpus.join("1"), "a")
        .file(corpus.join("2"), "ab")
        .file(corpus.join("3"), "abc")
        .file(corpus.join("4"), "abcd")
        .build();

    let corpus_count = || {
        fs::read_dir(project.root().join("fuzz").join("corpus").join("foo"))
            .unwrap()
            .map(|e| e.unwrap())
            .count()
    };
    assert_eq!(corpus_count(), 5);

    project
        .cargo_fuzz()
        .arg("cmin")
        .arg("foo")
        .assert()
        .success();
    assert_eq!(corpus_count(), 1);
}

#[test]
fn tmin() {
    let corpus = Path::new("fuzz").join("corpus").join("i_hate_zed");
    let test_case = corpus.join("test-case");
    let project = project("tmin")
        .with_fuzz()
        .fuzz_target(
            "i_hate_zed",
            r#"
                #![no_main]
                use libfuzzer_sys::fuzz_target;

                fuzz_target!(|data: &[u8]| {
                    let s = String::from_utf8_lossy(data);
                    if s.contains('z') {
                        panic!("nooooooooo");
                    }
                });
            "#,
        )
        .file(&test_case, "pack my box with five dozen liquor jugs")
        .build();
    let test_case = project.root().join(test_case);
    project
        .cargo_fuzz()
        .arg("tmin")
        .arg("i_hate_zed")
        .arg(&test_case)
        .assert()
        .stderr(
            predicates::str::contains("CRASH_MIN: minimizing crash input: ")
                .and(predicate::str::contains("(1 bytes) caused a crash")),
        )
        .success();
}
