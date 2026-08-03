#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

mod common;

use std::fs;

use common::{git, run_turbo};

fn setup_unaffected_selected_fixture(dir: &std::path::Path) {
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
  "tasks": {
    "build": {
      "cache": false,
      "inputs": ["$TURBO_DEFAULT$"]
    },
    "test": {
      "cache": false,
      "dependsOn": ["^build"],
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
    fs::write(
        dir.join("packages/alpha/package.json"),
        r#"{
  "name": "alpha",
  "version": "1.0.0",
  "dependencies": {
    "beta": "*"
  },
  "scripts": {
    "test": "node -e \"console.log('alpha test')\""
  }
}
"#,
    )
    .unwrap();
    fs::write(dir.join("packages/alpha/index.ts"), "export const value = 1;\n").unwrap();
    fs::write(
        dir.join("packages/beta/package.json"),
        r#"{
  "name": "beta",
  "version": "1.0.0",
  "scripts": {
    "build": "node -e \"console.log('beta build')\"",
    "test": "node -e \"console.log('beta test')\""
  }
}
"#,
    )
    .unwrap();
    fs::write(dir.join("packages/beta/index.ts"), "export const value = 1;\n").unwrap();
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
      "version": "1.0.0",
      "dependencies": {
        "beta": "*"
      }
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

#[test]
fn filtering_to_unaffected_package_does_not_keep_dependency_only_tasks() {
    let tempdir = tempfile::tempdir().unwrap();
    setup_unaffected_selected_fixture(tempdir.path());

    fs::write(
        tempdir.path().join("packages/alpha/index.ts"),
        "export const value = 2;\n",
    )
    .unwrap();

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

    assert_eq!(json["packages"], serde_json::json!(["beta"]));
    assert!(
        json["tasks"].as_array().expect("tasks array").is_empty(),
        "dependency-only beta#build must not become a selected affected root: {}",
        json["tasks"]
    );
}
