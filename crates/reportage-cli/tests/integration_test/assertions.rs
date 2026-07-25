use super::*;

// Representative assertion pass/fail scenarios live in e2e/assertions/.
// The tests here verify filesystem and byte boundaries and stable diagnostics.

#[test]
fn file_exists_fails_for_a_directory() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "directory is not a file" {
  $ mkdir -p a-directory
  assert {
    file <"a-directory"> exists
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "it is not a regular file (e.g. a directory)",
        ));
}

#[test]
fn file_exists_follows_symlink_to_regular_file() {
    #[cfg(unix)]
    {
        let dir = TempDir::new().unwrap();
        let script = write_script(
            &dir,
            "test.repor",
            r#"
case "symlink to file" {
  write <"real.txt"> ```
    hi
    ```
  $ ln -s real.txt link.txt
  assert {
    file <"link.txt"> exists
  }
}
"#,
        );
        reportage(&dir).arg(script).assert().code(0);
    }
}

#[test]
fn file_contains_fails_for_directory() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "file contains directory" {
  $ mkdir -p a-directory
  assert {
    file <"a-directory"> contains "anything"
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "it is not a regular file (e.g. a directory)",
        ));
}

#[test]
#[cfg(unix)]
fn file_contains_fails_for_non_utf8_content() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "file contains non-utf8" {
  $ printf '\377\376\000\377' > binary.dat
  assert {
    file <"binary.dat"> contains "anything"
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains("its content is not valid UTF-8"));
}

// The combined-evidence pattern (a process expectation alongside `file exists` and
// `file contains` in one assertion block) is covered by
// e2e/artifacts/file-assertion-evidence.repor.

// --- `contents_equals` assertions (#87) ---
//
// The representative workspace-file pass scenario lives in
// e2e/assertions/contents-equals.repor ("file contents_equals passes against a workspace
// expected file").

#[test]
fn file_contents_equals_fails_on_byte_mismatch_against_workspace_expected_file() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "file contents_equals workspace mismatch" {
  $ printf hello > expected.txt
  $ printf world > actual.txt
  assert {
    file <"actual.txt"> contents_equals <"expected.txt">
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "assertion.file.contents_equals.mismatch",
        ));
}

#[test]
fn file_contents_equals_fails_on_byte_mismatch_against_fixture_expected_file() {
    let dir = TempDir::new().unwrap();
    dir.child("expected.txt").write_str("hello").unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "file contents_equals fixture mismatch" {
  $ printf world > actual.txt
  assert {
    file <"actual.txt"> contents_equals @"expected.txt"
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(1);
}

#[test]
fn file_contents_equals_missing_actual_is_assertion_failure() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "file contents_equals missing actual" {
  $ printf hello > expected.txt
  assert {
    file <"does-not-exist.txt"> contents_equals <"expected.txt">
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "assertion.file.contents_equals.actual_missing",
        ));
}

// The missing-expected-workspace-path script error is covered by e2e/assertions/contents-equals.repor ("file contents_equals reports a script error for a missing expected workspace path"), which checks the same `semantic.file_contents_reference.missing` diagnostic code.

#[test]
fn file_contents_equals_missing_fixture_is_a_script_error() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "file contents_equals missing fixture" {
  $ printf hello > actual.txt
  assert {
    file <"actual.txt"> contents_equals @"does-not-exist.txt"
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(2)
        .stderr(predicates::str::contains(
            "semantic.fixture_reference.missing",
        ));
}

// The stdout-vs-fixture pass scenario is covered by e2e/assertions/contents-equals.repor
// ("stdout contents_equals passes against a fixture reference").

#[test]
fn stderr_contents_equals_fails_on_mismatch_against_workspace_expected_file() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "stderr contents_equals workspace mismatch" {
  $ printf oops > expected.txt
  $ printf nope 1>&2
  assert {
    stderr contents_equals <"expected.txt">
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "assertion.stderr.contents_equals.mismatch",
        ));
}

#[test]
fn stdout_contents_equals_fails_on_mismatch_against_workspace_expected_file() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "stdout contents_equals workspace mismatch" {
  $ printf hello > expected.txt
  $ printf world
  assert {
    stdout contents_equals <"expected.txt">
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "assertion.stdout.contents_equals.mismatch",
        ));
}

#[test]
fn file_contents_equals_actual_directory_is_assertion_failure() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "file contents_equals actual is a directory" {
  $ mkdir -p a-dir
  $ printf hello > expected.txt
  assert {
    file <"a-dir"> contents_equals <"expected.txt">
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "assertion.file.contents_equals.actual_not_regular_file",
        ));
}

// --- `text_equals` assertions (#88) ---
//
// Representative pass scenarios (quoted-string and heredoc literals) and the quoted-string
// mismatch scenario live in e2e/assertions/text-equals.repor.

#[test]
fn file_text_equals_fails_on_heredoc_byte_mismatch() {
    // Mirrors `file_text_equals_fails_on_byte_mismatch`, but with a heredoc expected value: a
    // failing heredoc-form text_equals must report the same diagnostic code as the quoted-string
    // form, and its human-rendered subject description must use the heredoc literal label
    // instead of the compact quoted-literal rendering (see `format_text_equals_source`).
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "file text_equals heredoc mismatch" {
  $ printf 'hello\nworld\n' > actual.txt
  assert {
    file <"actual.txt"> text_equals ```
    hello
    WORLD
    ```
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "assertion.file.text_equals.mismatch",
        ))
        .stderr(predicates::str::contains(
            "text_equals <heredoc literal> — bytes differ",
        ));
}

// The missing-actual-file assertion failure is covered by e2e/assertions/text-equals.repor
// ("file text_equals reports an assertion failure for a missing actual file"), which checks
// the same `assertion.file.text_equals.actual_missing` diagnostic code.

#[test]
fn file_text_equals_actual_directory_is_assertion_failure() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "file text_equals actual is a directory" {
  $ mkdir -p a-dir
  assert {
    file <"a-dir"> text_equals "hello"
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "assertion.file.text_equals.actual_not_regular_file",
        ));
}

// Both `text_equals` kind-mismatch script errors (rejecting a fixture reference and rejecting
// a workspace path literal as the expected value) are covered by e2e/assertions/text-equals.repor,
// which checks the same `semantic.literal.kind_mismatch` diagnostic code for each case.

// --- stdout / stderr `text_equals` assertions ---
//
// Representative pass scenarios (quoted-string and heredoc literals), the quoted-string
// mismatch scenarios, and both kind-mismatch script errors live in
// e2e/assertions/text-equals.repor.

#[test]
fn stdout_text_equals_fails_on_heredoc_byte_mismatch() {
    // Mirrors `file_text_equals_fails_on_heredoc_byte_mismatch` for a captured stream: a failing
    // heredoc-form stdout text_equals must report its own stream-scoped diagnostic code, and its
    // human-rendered subject description must use the `text_equals` operator keyword and the
    // heredoc literal label (see `format_text_equals_source` / `print_byte_comparison_detail`).
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "stdout text_equals heredoc mismatch" {
  $ printf 'hello\nworld\n'
  assert {
    stdout text_equals ```
    hello
    WORLD
    ```
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "assertion.stdout.text_equals.mismatch",
        ))
        .stderr(predicates::str::contains(
            "stdout text_equals <heredoc literal> — bytes differ",
        ));
}

#[test]
fn stderr_text_equals_mismatch_reports_stream_scoped_code_and_quoted_source() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "stderr text_equals quoted mismatch" {
  $ sh -c 'printf "warn\n" >&2'
  assert {
    stderr text_equals "other\n"
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "assertion.stderr.text_equals.mismatch",
        ))
        .stderr(predicates::str::contains(
            "stderr text_equals \"other\\n\" — bytes differ",
        ));
}

// --- dir assertions (#66) ---
//
// Representative pass/fail scenarios for `dir exists` and `dir contains` live in
// e2e/artifacts/dir-assertion-evidence.repor. The tests below verify diagnostic codes not
// covered there (missing path, not-a-directory, broken symlink), or additional source-path
// attribution alongside a diagnostic code (absolute/dot-segment rejection) that the self-test
// already checks without the source-path assertion.

#[test]
fn dir_exists_fails_against_a_regular_file() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "dir exists against a file" {
  $ touch marker
  assert {
    dir <"marker"> exists
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "it is not a directory (e.g. a regular file)",
        ))
        .stderr(predicates::str::contains(
            "assertion.dir.exists.not_directory",
        ));
}

#[test]
fn dir_exists_fails_for_a_missing_path() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "dir exists missing" {
  $ true
  assert {
    dir <"nope"> exists
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains("assertion.dir.exists.missing"));
}

#[test]
#[cfg(unix)]
fn dir_exists_fails_for_a_broken_symlink() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "broken symlink" {
  $ ln -s does-not-exist link
  assert {
    dir <"link"> exists
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains("assertion.dir.exists.missing"));
}
