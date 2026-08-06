#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

mod common;

use std::fs;

use common::{git, run_turbo, run_turbo_with_env};

fn setup_package_task_fixture(dir: &std::path::Path, filter_using_tasks: bool) {
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
        serde_json::to_string_pretty(&serde_json::json!({
            "$schema": "https://turborepo.com/schema.json",
            "tasks": {
                "build": {
                    "cache": false,
                    "inputs": ["$TURBO_DEFAULT$"]
                },
                "test": {
                    "cache": false,
                    "inputs": ["$TURBO_DEFAULT$"]
                }
            },
            "futureFlags": {
                "affectedUsingTaskInputs": true,
                "filterUsingTasks": filter_using_tasks
            }
        }))
        .unwrap(),
    )
    .unwrap();
    fs::write(
        dir.join("packages/alpha/package.json"),
        r#"{
  "name": "alpha",
  "version": "1.0.0",
  "scripts": {
    "build": "node -e \"console.log('alpha build')\""
  }
}
"#,
    )
    .unwrap();
    fs::write(
        dir.join("packages/alpha/index.ts"),
        "export const alpha = 1;\n",
    )
    .unwrap();
    fs::write(
        dir.join("packages/beta/package.json"),
        r#"{
  "name": "beta",
  "version": "1.0.0",
  "scripts": {
    "test": "node -e \"console.log('beta test')\""
  }
}
"#,
    )
    .unwrap();
    fs::write(
        dir.join("packages/beta/index.ts"),
        "export const beta = 1;\n",
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

fn package_task_dry_run(dir: &std::path::Path) -> serde_json::Value {
    fs::write(
        dir.join("packages/beta/index.ts"),
        "export const beta = 2;\n",
    )
    .unwrap();

    let output = run_turbo(
        dir,
        &[
            "run",
            "test",
            "alpha#build",
            "--affected",
            "--filter=beta",
            "--dry=json",
        ],
    );
    assert!(
        output.status.success(),
        "dry run should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout)
        .unwrap_or_else(|err| panic!("failed to parse dry-run JSON: {err}\nstdout: {stdout}"))
}

fn package_task_fail_open_dry_run(dir: &std::path::Path) -> serde_json::Value {
    let output = run_turbo_with_env(
        dir,
        &[
            "run",
            "test",
            "alpha#build",
            "--affected",
            "--filter=beta",
            "--dry=json",
        ],
        &[("TURBO_SCM_BASE", "definitely-missing-round-004-ref")],
    );
    assert!(
        output.status.success(),
        "fail-open dry run should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout)
        .unwrap_or_else(|err| panic!("failed to parse fail-open JSON: {err}\nstdout: {stdout}"))
}

fn assert_package_task_contract(json: &serde_json::Value) {
    let mut task_ids: Vec<&str> = json["tasks"]
        .as_array()
        .expect("tasks array")
        .iter()
        .map(|task| task["taskId"].as_str().expect("taskId string"))
        .collect();
    task_ids.sort_unstable();

    assert_eq!(
        task_ids,
        vec!["alpha#build", "beta#test"],
        "the package filter scopes the unqualified task without dropping the explicitly requested \
         package task"
    );
    assert_eq!(json["packages"], serde_json::json!(["beta"]));
}

#[test]
fn combined_task_filter_preserves_package_qualified_task() {
    let tempdir = tempfile::tempdir().unwrap();
    setup_package_task_fixture(tempdir.path(), true);

    assert_package_task_contract(&package_task_dry_run(tempdir.path()));
}

#[test]
fn task_input_affected_preserves_package_qualified_task() {
    let tempdir = tempfile::tempdir().unwrap();
    setup_package_task_fixture(tempdir.path(), false);

    assert_package_task_contract(&package_task_dry_run(tempdir.path()));
}

#[test]
fn task_input_affected_fail_open_preserves_package_qualified_task() {
    let tempdir = tempfile::tempdir().unwrap();
    setup_package_task_fixture(tempdir.path(), false);

    assert_package_task_contract(&package_task_fail_open_dry_run(tempdir.path()));
}
