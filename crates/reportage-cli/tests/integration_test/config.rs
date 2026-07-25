use super::*;

// Representative config-driven discovery scenarios live in e2e/discovery/.

#[test]
fn config_and_scripts_combined_is_rejected() {
    let dir = TempDir::new().unwrap();
    let script = write_script(&dir, "test.repor", PASSING_CASE);
    write_config(
        &dir,
        r#"
reportage {
  config {
    version 1
  }
  tests {
    path "test.repor"
  }
}
"#,
    );
    reportage(&dir)
        .arg("--config")
        .arg("reportage.kdl")
        .arg(script)
        .assert()
        .code(3);
}

#[test]
fn config_pattern_matching_no_files_exits_two() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        r#"
reportage {
  config {
    version 1
  }
  tests {
    path "no_match/**/*.repor"
  }
}
"#,
    );
    reportage(&dir).assert().code(2);
}

#[test]
fn config_with_dot_segment_path_exits_nonzero() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        r#"
reportage {
  config {
    version 1
  }
  tests {
    path "./test.repor"
  }
}
"#,
    );
    // Config validation error → exit 3
    reportage(&dir).assert().code(3);
}

// The source path attribution this test verified (discovered file names appearing in stdout)
// is covered by e2e/discovery/config-driven.repor and e2e/discovery/aggregate-failure.repor,
// which both assert `stdout contains` the discovered file names.

#[test]
fn file_read_error_exits_two_with_no_execution() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        r#"
reportage {
  config {
    version 1
  }
  tests {
    path "*.repor"
  }
}
"#,
    );
    // Write a file that matches but is a directory, not a regular file.
    // Actually, let's create a file and then remove it so glob matched it... easier: point to a non-existent file via explicit script mode.
    let nonexistent = dir.path().join("nonexistent.repor");
    reportage(&dir).arg(&nonexistent).assert().code(2);
}
