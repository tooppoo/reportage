//! Format-independent command outcome and its text / JSON renderings.
//!
//! Commands in this crate build a [`Report`] and never decide how it is displayed. The
//! rendering functions here are the only place that knows about streams, text layout, or the
//! JSON envelope, which keeps `--format json` from leaking format branches into the schema
//! artifact logic.
//!
//! The envelope shape is fixed by four rules the rest of the crate depends on: exactly one
//! envelope per run, a successful envelope on stdout and a failed one on stderr with the other
//! stream empty, an `error.category` that agrees with the process exit code, and a concrete
//! `recovery` whenever `recoverable` is true.

use serde_json::{Value, json};

/// Version of the JSON envelope contract emitted by [`render`].
const ENVELOPE_SCHEMA_VERSION: &str = "1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
}

/// The broad kind of failure, deliberately coarser than a [`Cause`]'s code so that related
/// causes share one exit code. The numeric mapping is part of this crate's documented contract:
/// CI and `just` recipes branch on the process exit code, not on message text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureCategory {
    /// Invalid command line. Produced by the argument parser, not by command logic.
    Usage,
    /// The internal source schema is unusable as input: malformed JSON, or annotations in
    /// locations the allowlist does not permit.
    Input,
    /// A schema file could not be read or written.
    Filesystem,
    /// The committed public schema disagrees with what the internal source schema generates.
    Conflict,
    /// A violated invariant inside this tool.
    Internal,
}

impl FailureCategory {
    pub fn exit_code(self) -> i32 {
        match self {
            FailureCategory::Internal => 1,
            FailureCategory::Usage => 2,
            FailureCategory::Input => 3,
            FailureCategory::Filesystem => 4,
            FailureCategory::Conflict => 5,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            FailureCategory::Usage => "usage",
            FailureCategory::Input => "input",
            FailureCategory::Filesystem => "filesystem",
            FailureCategory::Conflict => "conflict",
            FailureCategory::Internal => "internal",
        }
    }
}

/// One specific reason a command failed.
///
/// Every cause names both halves of a contract's schema pair even though only one of them is
/// the anchor, because a maintainer reading a schema artifact diagnostic always needs to know
/// which source produced which output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cause {
    pub code: &'static str,
    pub message: String,
    pub contract: &'static str,
    /// The file to look at first: the internal source for input problems, the generated public
    /// schema for staleness and write problems.
    pub path: String,
    pub internal_schema_path: String,
    pub public_schema_path: String,
    /// JSON Pointer of the offending location inside `path`, when one is known.
    pub pointer: Option<String>,
    /// One-based source position inside `path`, when the failure is positional rather than
    /// structural.
    pub position: Option<(usize, usize)>,
}

impl Cause {
    pub fn new(
        code: &'static str,
        message: String,
        contract: &'static str,
        path: String,
        internal_schema_path: String,
        public_schema_path: String,
    ) -> Self {
        Self {
            code,
            message,
            contract,
            path,
            internal_schema_path,
            public_schema_path,
            pointer: None,
            position: None,
        }
    }

    pub fn with_pointer(mut self, pointer: String) -> Self {
        self.pointer = Some(pointer);
        self
    }

    pub fn with_position(mut self, line: usize, column: usize) -> Self {
        self.position = Some((line, column));
        self
    }

    fn to_json(&self) -> Value {
        let mut value = json!({
            "code": self.code,
            "message": self.message,
            "contract": self.contract,
            "path": self.path,
            "internalSchemaPath": self.internal_schema_path,
            "publicSchemaPath": self.public_schema_path,
        });
        if let Some(pointer) = &self.pointer {
            value["pointer"] = json!(pointer);
        }
        if let Some((line, column)) = self.position {
            value["line"] = json!(line);
            value["column"] = json!(column);
        }
        value
    }

    fn to_text(&self) -> String {
        let mut lines = vec![format!("  - [{}] {}", self.code, self.message)];
        if let Some(pointer) = &self.pointer {
            lines.push(format!("    location:               {pointer}"));
        }
        if let Some((line, column)) = self.position {
            lines.push(format!(
                "    location:               line {line}, column {column}"
            ));
        }
        lines.push(format!(
            "    internal source schema: {}",
            self.internal_schema_path
        ));
        lines.push(format!(
            "    public schema:          {}",
            self.public_schema_path
        ));
        lines.join("\n")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandError {
    pub code: &'static str,
    pub category: FailureCategory,
    pub message: String,
    /// Concrete next action. `None` marks the failure as not user-recoverable.
    pub recovery: Option<String>,
    pub causes: Vec<Cause>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileAction {
    Create,
    Modify,
}

impl FileAction {
    fn as_str(self) -> &'static str {
        match self {
            FileAction::Create => "create",
            FileAction::Modify => "modify",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileState {
    Planned,
    Completed,
}

impl FileState {
    fn as_str(self) -> &'static str {
        match self {
            FileState::Planned => "planned",
            FileState::Completed => "completed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    pub action: FileAction,
    pub path: String,
    pub state: FileState,
}

impl FileChange {
    fn to_json(&self) -> Value {
        json!({
            "action": self.action.as_str(),
            "path": self.path,
            "state": self.state.as_str(),
        })
    }
}

/// A completed command, before any decision about how to display it.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub command: &'static str,
    pub dry_run: bool,
    /// Files the command created or modified, or planned to under `--dry-run`. Populated even
    /// when the command fails, so a partial mutation is never hidden by the failure.
    pub file_changes: Vec<FileChange>,
    pub body: ReportBody,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReportBody {
    Success {
        /// Command-specific `result` object of the JSON envelope.
        result: Value,
        /// Human-facing success lines, in display order.
        summary: Vec<String>,
    },
    Failure(CommandError),
}

impl Report {
    pub fn error(&self) -> Option<&CommandError> {
        match &self.body {
            ReportBody::Success { .. } => None,
            ReportBody::Failure(error) => Some(error),
        }
    }

    pub fn exit_code(&self) -> i32 {
        match &self.body {
            ReportBody::Success { .. } => 0,
            ReportBody::Failure(error) => error.category.exit_code(),
        }
    }
}

/// What a command writes to each stream, and the status it exits with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

pub fn render(report: &Report, format: OutputFormat) -> Rendered {
    match format {
        OutputFormat::Text => render_text(report),
        OutputFormat::Json => render_json(report),
    }
}

fn render_text(report: &Report) -> Rendered {
    let mut lines = Vec::new();
    let stream_is_stdout = report.error().is_none();

    match &report.body {
        ReportBody::Success { summary, .. } => lines.extend(summary.iter().cloned()),
        ReportBody::Failure(error) => {
            lines.push(format!("error[{}]: {}", error.code, error.message));
            lines.extend(error.causes.iter().map(Cause::to_text));
        }
    }

    lines.extend(report.file_changes.iter().map(|change| {
        let marker = match change.state {
            FileState::Planned => "would",
            FileState::Completed => "did",
        };
        format!("{marker} {} {}", change.action.as_str(), change.path)
    }));

    if let Some(recovery) = report.error().and_then(|error| error.recovery.as_ref()) {
        lines.push(format!("recovery: {recovery}"));
    }

    let text = if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    };

    if stream_is_stdout {
        Rendered {
            stdout: text,
            stderr: String::new(),
            exit_code: 0,
        }
    } else {
        Rendered {
            stdout: String::new(),
            stderr: text,
            exit_code: report.exit_code(),
        }
    }
}

fn render_json(report: &Report) -> Rendered {
    let exit_code = report.exit_code();
    let mut envelope = json!({
        "schemaVersion": ENVELOPE_SCHEMA_VERSION,
        "status": if report.error().is_some() { "error" } else { "success" },
        "command": report.command,
        "exitCode": exit_code,
        "dryRun": report.dry_run,
        "fileChanges": report.file_changes.iter().map(FileChange::to_json).collect::<Vec<_>>(),
        "warnings": [],
        "logs": [],
    });

    match &report.body {
        ReportBody::Success { result, .. } => {
            envelope["result"] = result.clone();
        }
        ReportBody::Failure(error) => {
            let mut rendered_error = json!({
                "code": error.code,
                "category": error.category.as_str(),
                "message": error.message,
                "recoverable": error.recovery.is_some(),
                "causes": error.causes.iter().map(Cause::to_json).collect::<Vec<_>>(),
            });
            if let Some(recovery) = &error.recovery {
                rendered_error["recovery"] = json!(recovery);
            }
            envelope["error"] = rendered_error;
        }
    }

    let text = format!(
        "{}\n",
        serde_json::to_string_pretty(&envelope).expect("envelope serialization is infallible")
    );

    if exit_code == 0 {
        Rendered {
            stdout: text,
            stderr: String::new(),
            exit_code,
        }
    } else {
        Rendered {
            stdout: String::new(),
            stderr: text,
            exit_code,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cause() -> Cause {
        Cause::new(
            "PUBLIC_SCHEMA_STALE",
            "regenerated output differs from the committed file".to_owned(),
            "json-report",
            "spec/output/json-report/schema.json".to_owned(),
            "spec/output/json-report/schema.internal.json".to_owned(),
            "spec/output/json-report/schema.json".to_owned(),
        )
    }

    fn failure_report() -> Report {
        Report {
            command: "schema-artifacts.check",
            dry_run: false,
            file_changes: Vec::new(),
            body: ReportBody::Failure(CommandError {
                code: "PUBLIC_SCHEMA_OUT_OF_DATE",
                category: FailureCategory::Conflict,
                message: "1 public schema is out of date.".to_owned(),
                recovery: Some("run `just schema-artifacts-gen`".to_owned()),
                causes: vec![cause()],
            }),
        }
    }

    fn success_report() -> Report {
        Report {
            command: "schema-artifacts.gen",
            dry_run: true,
            file_changes: vec![FileChange {
                action: FileAction::Modify,
                path: "spec/output/json-report/schema.json".to_owned(),
                state: FileState::Planned,
            }],
            body: ReportBody::Success {
                result: json!({ "contracts": 2 }),
                summary: vec!["1 public schema would change.".to_owned()],
            },
        }
    }

    #[test]
    fn exit_code_matches_category() {
        assert_eq!(success_report().exit_code(), 0);
        assert_eq!(failure_report().exit_code(), 5);
        assert_eq!(FailureCategory::Usage.exit_code(), 2);
        assert_eq!(FailureCategory::Input.exit_code(), 3);
        assert_eq!(FailureCategory::Filesystem.exit_code(), 4);
        assert_eq!(FailureCategory::Internal.exit_code(), 1);
    }

    #[test]
    fn text_success_goes_to_stdout() {
        let rendered = render(&success_report(), OutputFormat::Text);

        assert_eq!(rendered.exit_code, 0);
        assert!(rendered.stderr.is_empty());
        assert_eq!(
            rendered.stdout,
            "1 public schema would change.\nwould modify spec/output/json-report/schema.json\n"
        );
    }

    #[test]
    fn text_failure_goes_to_stderr_with_both_paths_and_recovery() {
        let rendered = render(&failure_report(), OutputFormat::Text);

        assert!(rendered.stdout.is_empty());
        assert_eq!(rendered.exit_code, 5);
        assert!(rendered.stderr.contains("error[PUBLIC_SCHEMA_OUT_OF_DATE]"));
        assert!(
            rendered
                .stderr
                .contains("internal source schema: spec/output/json-report/schema.internal.json")
        );
        assert!(
            rendered
                .stderr
                .contains("public schema:          spec/output/json-report/schema.json")
        );
        assert!(
            rendered
                .stderr
                .contains("recovery: run `just schema-artifacts-gen`")
        );
    }

    #[test]
    fn json_success_envelope_carries_result_and_no_error() {
        let rendered = render(&success_report(), OutputFormat::Json);

        assert!(rendered.stderr.is_empty());
        let envelope: Value = serde_json::from_str(&rendered.stdout).expect("valid JSON");
        assert_eq!(envelope["status"], "success");
        assert_eq!(envelope["schemaVersion"], "1");
        assert_eq!(envelope["command"], "schema-artifacts.gen");
        assert_eq!(envelope["exitCode"], 0);
        assert_eq!(envelope["dryRun"], true);
        assert_eq!(envelope["result"]["contracts"], 2);
        assert_eq!(envelope["fileChanges"][0]["state"], "planned");
        assert_eq!(envelope["fileChanges"][0]["action"], "modify");
        assert!(envelope.get("error").is_none());
        assert_eq!(envelope["warnings"], json!([]));
        assert_eq!(envelope["logs"], json!([]));
    }

    #[test]
    fn json_failure_envelope_goes_to_stderr_with_causes() {
        let rendered = render(&failure_report(), OutputFormat::Json);

        assert!(rendered.stdout.is_empty());
        let envelope: Value = serde_json::from_str(&rendered.stderr).expect("valid JSON");
        assert_eq!(envelope["status"], "error");
        assert_eq!(envelope["exitCode"], 5);
        assert_eq!(envelope["error"]["category"], "conflict");
        assert_eq!(envelope["error"]["recoverable"], true);
        assert_eq!(
            envelope["error"]["recovery"],
            "run `just schema-artifacts-gen`"
        );
        assert_eq!(
            envelope["error"]["causes"][0]["code"],
            "PUBLIC_SCHEMA_STALE"
        );
        assert_eq!(
            envelope["error"]["causes"][0]["internalSchemaPath"],
            "spec/output/json-report/schema.internal.json"
        );
        assert!(
            envelope["error"]["causes"][0]
                .get("pointer")
                .is_none_or(Value::is_null)
        );
        assert!(envelope.get("result").is_none());
    }

    #[test]
    fn json_cause_and_text_cause_include_the_pointer_when_known() {
        let mut report = failure_report();
        if let ReportBody::Failure(error) = &mut report.body {
            error.recovery = None;
            error.causes[0].pointer = Some("/$defs/Tool/x-reportage-snapshot".to_owned());
        }

        let text = render(&report, OutputFormat::Text);
        assert!(
            text.stderr
                .contains("location:               /$defs/Tool/x-reportage-snapshot")
        );
        assert!(!text.stderr.contains("recovery:"));

        let envelope: Value =
            serde_json::from_str(&render(&report, OutputFormat::Json).stderr).expect("valid JSON");
        assert_eq!(
            envelope["error"]["causes"][0]["pointer"],
            "/$defs/Tool/x-reportage-snapshot"
        );
        assert_eq!(envelope["error"]["recoverable"], false);
        assert!(envelope["error"].get("recovery").is_none());
    }

    #[test]
    fn a_positional_cause_reports_line_and_column() {
        let mut report = failure_report();
        if let ReportBody::Failure(error) = &mut report.body {
            error.causes[0] = cause().with_position(12, 5);
        }

        assert!(
            render(&report, OutputFormat::Text)
                .stderr
                .contains("location:               line 12, column 5")
        );

        let envelope: Value =
            serde_json::from_str(&render(&report, OutputFormat::Json).stderr).expect("valid JSON");
        assert_eq!(envelope["error"]["causes"][0]["line"], 12);
        assert_eq!(envelope["error"]["causes"][0]["column"], 5);
    }

    #[test]
    fn text_render_of_an_empty_success_writes_nothing() {
        let report = Report {
            command: "schema-artifacts.check",
            dry_run: false,
            file_changes: Vec::new(),
            body: ReportBody::Success {
                result: json!({}),
                summary: Vec::new(),
            },
        };

        assert_eq!(render(&report, OutputFormat::Text).stdout, "");
    }
}
