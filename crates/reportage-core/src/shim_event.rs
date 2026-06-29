use std::path::{Path, PathBuf};

/// Environment variable set by the runner before each action execution.
///
/// Protocol-compliant shims read this variable to locate the action-scoped
/// event directory and write invocation event files into it.
pub const SHIM_EVENT_DIR_VAR: &str = "REPORTAGE_SHIM_EVENT_DIR";

/// Error encountered while parsing a shim event file.
#[derive(Debug)]
pub enum ShimEventParseError {
    /// The file content is not valid JSON.
    InvalidJson(String),
    /// A required field is absent from the event object.
    MissingField(String),
    /// A field is present but has the wrong type.
    InvalidFieldType {
        field: String,
        expected: &'static str,
    },
    /// The `schema_version` value is not recognized.
    UnsupportedSchemaVersion(u64),
    /// The `event` value is not recognized.
    UnknownEvent(String),
}

impl std::fmt::Display for ShimEventParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShimEventParseError::InvalidJson(msg) => write!(f, "invalid JSON: {msg}"),
            ShimEventParseError::MissingField(field) => {
                write!(f, "missing required field '{field}'")
            }
            ShimEventParseError::InvalidFieldType { field, expected } => {
                write!(f, "field '{field}' must be {expected}")
            }
            ShimEventParseError::UnsupportedSchemaVersion(v) => {
                write!(f, "unsupported schema_version {v}")
            }
            ShimEventParseError::UnknownEvent(e) => {
                write!(f, "unknown event '{e}'")
            }
        }
    }
}

impl std::error::Error for ShimEventParseError {}

/// The target executable invocation recorded in a shim event.
///
/// Models the invocation as `program` plus fixed `args`, matching the
/// `ExecutableInvocation` shape used when materializing shims. This allows
/// targets such as `ruby tool.rb` to be represented without changing the model.
#[derive(Debug, Clone)]
pub struct ShimInvocationTarget {
    pub program: PathBuf,
    pub args: Vec<String>,
}

/// A shim invocation event written by a protocol-compliant shim to the
/// action-scoped event directory before delegating to its target invocation.
///
/// The runner reads these events after the action completes and attaches them
/// to the corresponding `ActionResult`. Absence means no protocol-compliant
/// shim was observed; it does not prove that no shim or wrapper was involved.
#[derive(Debug, Clone)]
pub struct ShimInvocationEvent {
    pub schema_version: u64,
    pub command_name: String,
    pub shim_path: PathBuf,
    pub target: ShimInvocationTarget,
    pub forwards_caller_args: bool,
}

/// Parse a shim event file from its JSON content.
///
/// Returns an error if the content is malformed; the caller decides how to
/// surface the error. Does not panic on unexpected content.
pub fn parse_event_file(content: &str) -> Result<ShimInvocationEvent, ShimEventParseError> {
    let value: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| ShimEventParseError::InvalidJson(e.to_string()))?;

    let obj = value
        .as_object()
        .ok_or_else(|| ShimEventParseError::InvalidFieldType {
            field: "<root>".to_string(),
            expected: "object",
        })?;

    let schema_version = obj
        .get("schema_version")
        .ok_or_else(|| ShimEventParseError::MissingField("schema_version".to_string()))?
        .as_u64()
        .ok_or_else(|| ShimEventParseError::InvalidFieldType {
            field: "schema_version".to_string(),
            expected: "unsigned integer",
        })?;

    if schema_version != 1 {
        return Err(ShimEventParseError::UnsupportedSchemaVersion(
            schema_version,
        ));
    }

    let event = obj
        .get("event")
        .ok_or_else(|| ShimEventParseError::MissingField("event".to_string()))?
        .as_str()
        .ok_or_else(|| ShimEventParseError::InvalidFieldType {
            field: "event".to_string(),
            expected: "string",
        })?;

    if event != "shim_invoked" {
        return Err(ShimEventParseError::UnknownEvent(event.to_string()));
    }

    let command_name = obj
        .get("command_name")
        .ok_or_else(|| ShimEventParseError::MissingField("command_name".to_string()))?
        .as_str()
        .ok_or_else(|| ShimEventParseError::InvalidFieldType {
            field: "command_name".to_string(),
            expected: "string",
        })?
        .to_string();

    let shim_path = PathBuf::from(
        obj.get("shim_path")
            .ok_or_else(|| ShimEventParseError::MissingField("shim_path".to_string()))?
            .as_str()
            .ok_or_else(|| ShimEventParseError::InvalidFieldType {
                field: "shim_path".to_string(),
                expected: "string",
            })?,
    );

    let forwards_caller_args = obj
        .get("forwards_caller_args")
        .ok_or_else(|| ShimEventParseError::MissingField("forwards_caller_args".to_string()))?
        .as_bool()
        .ok_or_else(|| ShimEventParseError::InvalidFieldType {
            field: "forwards_caller_args".to_string(),
            expected: "boolean",
        })?;

    let target_obj = obj
        .get("target")
        .ok_or_else(|| ShimEventParseError::MissingField("target".to_string()))?
        .as_object()
        .ok_or_else(|| ShimEventParseError::InvalidFieldType {
            field: "target".to_string(),
            expected: "object",
        })?;

    let program = PathBuf::from(
        target_obj
            .get("program")
            .ok_or_else(|| ShimEventParseError::MissingField("target.program".to_string()))?
            .as_str()
            .ok_or_else(|| ShimEventParseError::InvalidFieldType {
                field: "target.program".to_string(),
                expected: "string",
            })?,
    );

    let args = target_obj
        .get("args")
        .ok_or_else(|| ShimEventParseError::MissingField("target.args".to_string()))?
        .as_array()
        .ok_or_else(|| ShimEventParseError::InvalidFieldType {
            field: "target.args".to_string(),
            expected: "array",
        })?
        .iter()
        .enumerate()
        .map(|(i, v)| {
            v.as_str()
                .ok_or_else(|| ShimEventParseError::InvalidFieldType {
                    field: format!("target.args[{i}]"),
                    expected: "string",
                })
                .map(|s| s.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ShimInvocationEvent {
        schema_version,
        command_name,
        shim_path,
        target: ShimInvocationTarget { program, args },
        forwards_caller_args,
    })
}

/// Read and parse all shim event files from `dir`.
///
/// Files that cannot be read or parsed are returned as warning strings rather
/// than hard errors so that malformed events do not silently corrupt the action
/// result. Non-`.json` files in the directory are ignored.
pub fn collect_from_dir(dir: &Path) -> (Vec<ShimInvocationEvent>, Vec<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            return (
                vec![],
                vec![format!(
                    "failed to read shim event directory '{}': {e}",
                    dir.display()
                )],
            );
        }
    };

    let mut events: Vec<ShimInvocationEvent> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                warnings.push(format!("failed to read shim event directory entry: {e}"));
                continue;
            }
        };

        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                warnings.push(format!(
                    "failed to read shim event file '{}': {e}",
                    path.display()
                ));
                continue;
            }
        };

        match parse_event_file(&content) {
            Ok(event) => events.push(event),
            Err(e) => {
                warnings.push(format!(
                    "malformed shim event file '{}': {e}",
                    path.display()
                ));
            }
        }
    }

    (events, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_event_file: valid events ---

    #[test]
    fn valid_event_with_no_args_parses() {
        let json = r#"{
            "schema_version": 1,
            "event": "shim_invoked",
            "command_name": "mytool",
            "shim_path": "/tmp/shims/mytool",
            "target": {
                "program": "/usr/bin/mytool",
                "args": []
            },
            "forwards_caller_args": true
        }"#;
        let event = parse_event_file(json).unwrap();
        assert_eq!(event.schema_version, 1);
        assert_eq!(event.command_name, "mytool");
        assert_eq!(event.shim_path, PathBuf::from("/tmp/shims/mytool"));
        assert_eq!(event.target.program, PathBuf::from("/usr/bin/mytool"));
        assert!(event.target.args.is_empty());
        assert!(event.forwards_caller_args);
    }

    #[test]
    fn valid_event_with_fixed_args_parses() {
        let json = r#"{
            "schema_version": 1,
            "event": "shim_invoked",
            "command_name": "ruby-tool",
            "shim_path": "/tmp/shims/ruby-tool",
            "target": {
                "program": "/usr/bin/ruby",
                "args": ["/scripts/tool.rb"]
            },
            "forwards_caller_args": true
        }"#;
        let event = parse_event_file(json).unwrap();
        assert_eq!(event.target.program, PathBuf::from("/usr/bin/ruby"));
        assert_eq!(event.target.args, vec!["/scripts/tool.rb"]);
    }

    #[test]
    fn forwards_caller_args_false_parses() {
        let json = r#"{
            "schema_version": 1,
            "event": "shim_invoked",
            "command_name": "mytool",
            "shim_path": "/tmp/shims/mytool",
            "target": {
                "program": "/usr/bin/mytool",
                "args": []
            },
            "forwards_caller_args": false
        }"#;
        let event = parse_event_file(json).unwrap();
        assert!(!event.forwards_caller_args);
    }

    // --- parse_event_file: error cases ---

    #[test]
    fn not_json_returns_invalid_json_error() {
        assert!(matches!(
            parse_event_file("not json"),
            Err(ShimEventParseError::InvalidJson(_))
        ));
    }

    #[test]
    fn json_array_at_root_returns_type_error() {
        assert!(matches!(
            parse_event_file("[]"),
            Err(ShimEventParseError::InvalidFieldType { .. })
        ));
    }

    #[test]
    fn missing_schema_version_returns_error() {
        let json = r#"{"event":"shim_invoked","command_name":"t","shim_path":"/s","target":{"program":"/p","args":[]},"forwards_caller_args":true}"#;
        assert!(matches!(
            parse_event_file(json),
            Err(ShimEventParseError::MissingField(f)) if f == "schema_version"
        ));
    }

    #[test]
    fn schema_version_two_returns_unsupported_error() {
        let json = r#"{"schema_version":2,"event":"shim_invoked","command_name":"t","shim_path":"/s","target":{"program":"/p","args":[]},"forwards_caller_args":true}"#;
        assert!(matches!(
            parse_event_file(json),
            Err(ShimEventParseError::UnsupportedSchemaVersion(2))
        ));
    }

    #[test]
    fn missing_event_field_returns_error() {
        let json = r#"{"schema_version":1,"command_name":"t","shim_path":"/s","target":{"program":"/p","args":[]},"forwards_caller_args":true}"#;
        assert!(matches!(
            parse_event_file(json),
            Err(ShimEventParseError::MissingField(f)) if f == "event"
        ));
    }

    #[test]
    fn unknown_event_type_returns_error() {
        let json = r#"{"schema_version":1,"event":"unknown_event","command_name":"t","shim_path":"/s","target":{"program":"/p","args":[]},"forwards_caller_args":true}"#;
        assert!(matches!(
            parse_event_file(json),
            Err(ShimEventParseError::UnknownEvent(e)) if e == "unknown_event"
        ));
    }

    #[test]
    fn missing_command_name_returns_error() {
        let json = r#"{"schema_version":1,"event":"shim_invoked","shim_path":"/s","target":{"program":"/p","args":[]},"forwards_caller_args":true}"#;
        assert!(matches!(
            parse_event_file(json),
            Err(ShimEventParseError::MissingField(f)) if f == "command_name"
        ));
    }

    #[test]
    fn missing_shim_path_returns_error() {
        let json = r#"{"schema_version":1,"event":"shim_invoked","command_name":"t","target":{"program":"/p","args":[]},"forwards_caller_args":true}"#;
        assert!(matches!(
            parse_event_file(json),
            Err(ShimEventParseError::MissingField(f)) if f == "shim_path"
        ));
    }

    #[test]
    fn missing_target_returns_error() {
        let json = r#"{"schema_version":1,"event":"shim_invoked","command_name":"t","shim_path":"/s","forwards_caller_args":true}"#;
        assert!(matches!(
            parse_event_file(json),
            Err(ShimEventParseError::MissingField(f)) if f == "target"
        ));
    }

    #[test]
    fn missing_target_program_returns_error() {
        let json = r#"{"schema_version":1,"event":"shim_invoked","command_name":"t","shim_path":"/s","target":{"args":[]},"forwards_caller_args":true}"#;
        assert!(matches!(
            parse_event_file(json),
            Err(ShimEventParseError::MissingField(f)) if f == "target.program"
        ));
    }

    #[test]
    fn missing_target_args_returns_error() {
        let json = r#"{"schema_version":1,"event":"shim_invoked","command_name":"t","shim_path":"/s","target":{"program":"/p"},"forwards_caller_args":true}"#;
        assert!(matches!(
            parse_event_file(json),
            Err(ShimEventParseError::MissingField(f)) if f == "target.args"
        ));
    }

    #[test]
    fn missing_forwards_caller_args_returns_error() {
        let json = r#"{"schema_version":1,"event":"shim_invoked","command_name":"t","shim_path":"/s","target":{"program":"/p","args":[]}}"#;
        assert!(matches!(
            parse_event_file(json),
            Err(ShimEventParseError::MissingField(f)) if f == "forwards_caller_args"
        ));
    }

    #[test]
    fn non_integer_schema_version_returns_type_error() {
        let json = r#"{"schema_version":"one","event":"shim_invoked","command_name":"t","shim_path":"/s","target":{"program":"/p","args":[]},"forwards_caller_args":true}"#;
        assert!(matches!(
            parse_event_file(json),
            Err(ShimEventParseError::InvalidFieldType { field, .. }) if field == "schema_version"
        ));
    }

    #[test]
    fn non_array_target_args_returns_type_error() {
        let json = r#"{"schema_version":1,"event":"shim_invoked","command_name":"t","shim_path":"/s","target":{"program":"/p","args":"not-array"},"forwards_caller_args":true}"#;
        assert!(matches!(
            parse_event_file(json),
            Err(ShimEventParseError::InvalidFieldType { field, .. }) if field == "target.args"
        ));
    }

    #[test]
    fn non_string_element_in_args_returns_type_error() {
        let json = r#"{"schema_version":1,"event":"shim_invoked","command_name":"t","shim_path":"/s","target":{"program":"/p","args":[42]},"forwards_caller_args":true}"#;
        assert!(matches!(
            parse_event_file(json),
            Err(ShimEventParseError::InvalidFieldType { field, .. }) if field == "target.args[0]"
        ));
    }

    // --- collect_from_dir ---

    #[test]
    fn empty_event_dir_returns_empty_collections() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let (events, warnings) = collect_from_dir(dir.path());
        assert!(events.is_empty());
        assert!(warnings.is_empty());
    }

    #[test]
    fn valid_event_file_is_collected() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let json = r#"{"schema_version":1,"event":"shim_invoked","command_name":"mytool","shim_path":"/tmp/shims/mytool","target":{"program":"/usr/bin/mytool","args":[]},"forwards_caller_args":true}"#;
        std::fs::write(dir.path().join("12345.json"), json).unwrap();

        let (events, warnings) = collect_from_dir(dir.path());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].command_name, "mytool");
        assert!(warnings.is_empty());
    }

    #[test]
    fn non_json_files_are_ignored() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("other.txt"), "ignored").unwrap();

        let (events, warnings) = collect_from_dir(dir.path());
        assert!(events.is_empty());
        assert!(warnings.is_empty());
    }

    #[test]
    fn multiple_event_files_are_all_collected() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let json = |name: &str| {
            format!(
                r#"{{"schema_version":1,"event":"shim_invoked","command_name":"{name}","shim_path":"/tmp/shims/{name}","target":{{"program":"/usr/bin/{name}","args":[]}},"forwards_caller_args":true}}"#
            )
        };
        std::fs::write(dir.path().join("1.json"), json("tool-a")).unwrap();
        std::fs::write(dir.path().join("2.json"), json("tool-b")).unwrap();

        let (events, warnings) = collect_from_dir(dir.path());
        assert_eq!(events.len(), 2);
        assert!(warnings.is_empty());
    }

    #[test]
    fn malformed_event_file_produces_warning_not_error() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("bad.json"), "not valid json").unwrap();

        let (events, warnings) = collect_from_dir(dir.path());
        assert!(events.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("malformed shim event file"));
    }

    #[test]
    fn valid_and_malformed_files_in_same_dir_gives_partial_results() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let good_json = r#"{"schema_version":1,"event":"shim_invoked","command_name":"good","shim_path":"/s","target":{"program":"/p","args":[]},"forwards_caller_args":true}"#;
        std::fs::write(dir.path().join("good.json"), good_json).unwrap();
        std::fs::write(dir.path().join("bad.json"), "not json").unwrap();

        let (events, warnings) = collect_from_dir(dir.path());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].command_name, "good");
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn nonexistent_dir_returns_warning() {
        let (events, warnings) = collect_from_dir(Path::new("/nonexistent/shim/event/dir"));
        assert!(events.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("failed to read shim event directory"));
    }
}
