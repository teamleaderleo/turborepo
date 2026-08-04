#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

mod common;

use std::fs;

use common::{git, run_turbo};

fn setup_task_level_filter_fixture(dir: &std::path::Path) {
    fs::create_dir_all(dir.join("packages/alpha")).unwrap();
    fs::create_dir_all(dir.join("packages/beta")).unwrap();

    fs::write(
        dir.join("package.json"),
        r#"{
  "name": "root",
  "private": true,
  "packageManager": "npm@10.5.0",
  "workspaces": ["packages/*"]
}
"#,
    )
    .unwrap();
    fs::write(
        dir.join("turbo.json"),
        r#"{
  "$schema": "https://turborepo.com/schema.json",
  "globalDependencies": ["shared.txt"],
  "tasks": {
    "test": {
      "cache": false,
      "inputs": ["$TURBO_DEFAULT$"]
    }
  },
  "futureFlags": {
    "affectedUsingTaskInputs": true
  }
}
"#,
    )
    .unwrap();
    fs::write(dir.join("shared.txt"), "before\n").unwrap();
    fs::write(
        dir.join("packages/alpha/package.json"),
        r#"{
  "name": "alpha",
  "version": "1.0.0",
  "scripts": {
    "test": "node -e \"console.log('alpha')\""
  }
}
"#,
    )
    .unwrap();
    fs::write(
        dir.join("packages/beta/package.json"),
        r#"{
  "name": "beta",
  "version": "1.0.0",
  "scripts": {
    "test": "node -e \"console.log('beta')\""
  }
}
"#,
    )
    .unwrap();
    fs::write(
        dir.join("package-lock.json"),
        r#"{
  "name": "root",
  "lockfileVersion": 3,
  "requires": true,
  "packages": {
    "": {
      "name": "root",
      "workspaces": ["packages/*"]
    },
    "node_modules/alpha": {
      "resolved": "packages/alpha",
      "link": true
    },
    "node_modules/beta": {
      "resolved": "packages/beta",
      "link": true
    },
    "packages/alpha": {
      "name": "alpha",
      "version": "1.0.0"
    },
    "packages/beta": {
      "name": "beta",
      "version": "1.0.0"
    }
  }
}
"#,
    )
    .unwrap();

    git(dir, &["init", "--quiet", "--initial-branch=main"]);
    git(dir, &["config", "user.email", "turbo-test@example.com"]);
    git(dir, &["config", "user.name", "Turbo Test"]);
    git(dir, &["add", "."]);
    git(
        dir,
        &[
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-m",
            "Initial",
            "--quiet",
        ],
    );
    git(dir, &["checkout", "-b", "my-branch"]);
}

fn dry_run(tempdir: &std::path::Path, filter: &str, parallel: bool) -> serde_json::Value {
    let mut args = vec!["run", "test", "--affected", filter, "--dry=json"];
    if parallel {
        args.push("--parallel");
    }

    let output = run_turbo(tempdir, &args);
    assert!(
        output.status.success(),
        "dry run should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout)
        .unwrap_or_else(|err| panic!("failed to parse dry-run JSON: {err}\nstdout: {stdout}"))
}

fn task_ids(json: &serde_json::Value) -> Vec<&str> {
    json["tasks"]
        .as_array()
        .expect("tasks array")
        .iter()
        .map(|task| task["taskId"].as_str().expect("taskId string"))
        .collect()
}

#[test]
fn task_level_affected_intersects_package_filter() {
    let tempdir = tempfile::tempdir().unwrap();
    setup_task_level_filter_fixture(tempdir.path());

    fs::write(tempdir.path().join("shared.txt"), "after\n").unwrap();

    let json = dry_run(tempdir.path(), "--filter=beta", false);
    assert_eq!(
        task_ids(&json),
        vec!["beta#test"],
        "package filter and task-input affectedness must intersect"
    );
    assert_eq!(json["packages"], serde_json::json!(["beta"]));
}

#[test]
fn task_level_affected_intersects_package_filter_in_parallel_mode() {
    let tempdir = tempfile::tempdir().unwrap();
    setup_task_level_filter_fixture(tempdir.path());

    fs::write(tempdir.path().join("shared.txt"), "after\n").unwrap();

    let json = dry_run(tempdir.path(), "--filter=beta", true);
    assert_eq!(task_ids(&json), vec!["beta#test"]);
    assert_eq!(json["packages"], serde_json::json!(["beta"]));
}

#[test]
fn task_level_affected_respects_exclude_only_filter() {
    let tempdir = tempfile::tempdir().unwrap();
    setup_task_level_filter_fixture(tempdir.path());

    fs::write(tempdir.path().join("shared.txt"), "after\n").unwrap();

    let json = dry_run(tempdir.path(), "--filter=!alpha", false);
    assert_eq!(task_ids(&json), vec!["beta#test"]);
}
