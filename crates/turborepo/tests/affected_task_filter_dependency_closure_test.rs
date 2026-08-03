#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

mod common;

use std::fs;

use common::{git, run_turbo};

fn setup_dependency_closure_fixture(dir: &std::path::Path) {
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
      "dependsOn": ["^test"],
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
    "test": "node -e \"console.log('alpha test')\""
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
  "dependencies": {
    "alpha": "*"
  },
  "scripts": {
    "test": "node -e \"console.log('beta test')\""
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
      "version": "1.0.0",
      "dependencies": {
        "alpha": "*"
      }
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

#[test]
fn task_level_filter_keeps_same_name_dependency_tasks() {
    let tempdir = tempfile::tempdir().unwrap();
    setup_dependency_closure_fixture(tempdir.path());

    // The global dependency change affects both test entrypoints. Filtering to
    // beta must treat beta#test as the selected root while retaining alpha#test
    // because the selected task requires it through ^test.
    fs::write(tempdir.path().join("shared.txt"), "after\n").unwrap();

    let output = run_turbo(
        tempdir.path(),
        &[
            "run",
            "test",
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
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|err| panic!("failed to parse dry-run JSON: {err}\nstdout: {stdout}"));
    let mut task_ids: Vec<&str> = json["tasks"]
        .as_array()
        .expect("tasks array")
        .iter()
        .map(|task| task["taskId"].as_str().expect("taskId string"))
        .collect();
    task_ids.sort_unstable();

    assert_eq!(
        task_ids,
        vec!["alpha#test", "beta#test"],
        "selected roots and required same-name dependencies must remain distinct"
    );
    assert_eq!(json["packages"], serde_json::json!(["beta"]));
}
