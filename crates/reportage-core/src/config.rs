use kdl::KdlDocument;

use crate::shim::CommandName;

#[derive(Debug)]
pub struct ReportageConfig {
    pub commands: CommandsConfig,
    pub tests: TestsConfig,
}

/// Parsed `reportage.commands` block: registered command ids and their (still config-relative,
/// not yet resolved) `exec` targets. See docs/configuration.md — Commands.
#[derive(Debug, Default)]
pub struct CommandsConfig {
    pub commands: Vec<CommandConfig>,
}

#[derive(Debug, Clone)]
pub struct CommandConfig {
    pub id: String,
    /// Config-file-relative path, not yet resolved to an absolute executable target.
    /// Resolution against the config file's directory happens at run setup time, not here.
    /// See docs/configuration.md — Commands.
    pub exec: String,
}

#[derive(Debug)]
pub struct TestsConfig {
    pub paths: Vec<String>,
}

#[derive(Debug)]
pub enum ConfigError {
    ParseFailed(String),
    InvalidStructure(String),
    UnsupportedVersion(i128),
    InvalidPath(String),
    DuplicateCommandId(String),
    InvalidCommandId(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::ParseFailed(msg) => write!(f, "config parse error: {msg}"),
            ConfigError::InvalidStructure(msg) => write!(f, "invalid config: {msg}"),
            ConfigError::UnsupportedVersion(v) => {
                write!(
                    f,
                    "unsupported config version {v}; this build supports version 1"
                )
            }
            ConfigError::InvalidPath(path) => write!(
                f,
                "invalid path '{path}': absolute paths and dot segments (. or ..) are forbidden"
            ),
            ConfigError::DuplicateCommandId(id) => {
                write!(f, "duplicate command id '{id}': command ids must be unique")
            }
            ConfigError::InvalidCommandId(reason) => {
                write!(f, "invalid command id: {reason}")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

pub fn parse_config(source: &str) -> Result<ReportageConfig, ConfigError> {
    let doc: KdlDocument = source
        .parse()
        .map_err(|e: kdl::KdlError| ConfigError::ParseFailed(e.to_string()))?;

    let root = doc
        .get("reportage")
        .ok_or_else(|| ConfigError::InvalidStructure("missing 'reportage' root node".into()))?;

    let children = root.children().ok_or_else(|| {
        ConfigError::InvalidStructure("'reportage' node must have a block".into())
    })?;

    validate_version(children)?;
    let commands = parse_commands(children)?;
    let tests = parse_tests(children)?;

    Ok(ReportageConfig { commands, tests })
}

fn validate_version(children: &KdlDocument) -> Result<(), ConfigError> {
    let config_node = children
        .get("config")
        .ok_or_else(|| ConfigError::InvalidStructure("missing 'config' block".into()))?;

    let config_children = config_node
        .children()
        .ok_or_else(|| ConfigError::InvalidStructure("'config' node must have a block".into()))?;

    let version_node = config_children
        .get("version")
        .ok_or_else(|| ConfigError::InvalidStructure("missing 'version' in config block".into()))?;

    let version = version_node
        .entries()
        .iter()
        .find(|e| e.name().is_none())
        .and_then(|e| e.value().as_integer())
        .ok_or_else(|| {
            ConfigError::InvalidStructure("'version' must be an integer value".into())
        })?;

    if version != 1 {
        return Err(ConfigError::UnsupportedVersion(version));
    }

    Ok(())
}

/// Parses the optional `reportage.commands` block.
///
/// Unknown nodes inside `commands` or inside an individual `command` block are config errors
/// rather than silently ignored: a typo'd node (e.g. `exex` instead of `exec`) would otherwise
/// leave a command unregistered, silently falling through to the ambient `PATH` instead of the
/// intended shim. See docs/configuration.md — Commands.
fn parse_commands(children: &KdlDocument) -> Result<CommandsConfig, ConfigError> {
    let Some(commands_node) = children.get("commands") else {
        return Ok(CommandsConfig::default());
    };

    let commands_children = commands_node
        .children()
        .ok_or_else(|| ConfigError::InvalidStructure("'commands' node must have a block".into()))?;

    let mut seen_ids = std::collections::HashSet::new();
    let mut commands = Vec::new();

    for node in commands_children.nodes() {
        if node.name().value() != "command" {
            return Err(ConfigError::InvalidStructure(format!(
                "unknown node '{}' in 'commands' block",
                node.name().value()
            )));
        }

        let id = node
            .entries()
            .iter()
            .find(|e| e.name().is_none())
            .and_then(|e| e.value().as_string())
            .ok_or_else(|| {
                ConfigError::InvalidStructure("'command' must have a string id argument".into())
            })?
            .to_string();

        CommandName::new(&id).map_err(|e| ConfigError::InvalidCommandId(e.to_string()))?;

        if !seen_ids.insert(id.clone()) {
            return Err(ConfigError::DuplicateCommandId(id));
        }

        let command_children = node.children().ok_or_else(|| {
            ConfigError::InvalidStructure(format!("'command \"{id}\"' must have a block"))
        })?;

        let mut exec: Option<String> = None;
        for exec_node in command_children.nodes() {
            if exec_node.name().value() != "exec" {
                return Err(ConfigError::InvalidStructure(format!(
                    "unknown node '{}' in 'command \"{id}\"' block",
                    exec_node.name().value()
                )));
            }
            if exec.is_some() {
                return Err(ConfigError::InvalidStructure(format!(
                    "'command \"{id}\"' must have exactly one 'exec' node"
                )));
            }

            let exec_value = exec_node
                .entries()
                .iter()
                .find(|e| e.name().is_none())
                .and_then(|e| e.value().as_string())
                .ok_or_else(|| {
                    ConfigError::InvalidStructure(format!(
                        "'exec' in 'command \"{id}\"' must have a string value"
                    ))
                })?
                .to_string();
            validate_path_value(&exec_value)?;
            exec = Some(exec_value);
        }

        let exec = exec.ok_or_else(|| {
            ConfigError::InvalidStructure(format!("'command \"{id}\"' is missing 'exec'"))
        })?;

        commands.push(CommandConfig { id, exec });
    }

    Ok(CommandsConfig { commands })
}

fn parse_tests(children: &KdlDocument) -> Result<TestsConfig, ConfigError> {
    let tests_node = children
        .get("tests")
        .ok_or_else(|| ConfigError::InvalidStructure("missing 'tests' block".into()))?;

    let tests_children = tests_node
        .children()
        .ok_or_else(|| ConfigError::InvalidStructure("'tests' node must have a block".into()))?;

    let paths: Result<Vec<String>, ConfigError> = tests_children
        .nodes()
        .iter()
        .filter(|n| n.name().value() == "path")
        .map(|n| {
            let value = n
                .entries()
                .iter()
                .find(|e| e.name().is_none())
                .and_then(|e| e.value().as_string())
                .ok_or_else(|| {
                    ConfigError::InvalidStructure("'path' must have a string value".into())
                })?;
            let path = value.to_string();
            validate_path_value(&path)?;
            Ok(path)
        })
        .collect();

    Ok(TestsConfig { paths: paths? })
}

fn validate_path_value(path: &str) -> Result<(), ConfigError> {
    if path.starts_with('/') {
        return Err(ConfigError::InvalidPath(path.to_string()));
    }
    for segment in path.split('/') {
        if segment == "." || segment == ".." {
            return Err(ConfigError::InvalidPath(path.to_string()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_config() {
        let src = r#"
reportage {
  config {
    version 1
  }
  tests {
    path "examples/**/*.repor"
  }
}
"#;
        let config = parse_config(src).unwrap();
        assert_eq!(config.tests.paths, vec!["examples/**/*.repor"]);
    }

    #[test]
    fn multiple_path_entries() {
        let src = r#"
reportage {
  config {
    version 1
  }
  tests {
    path "e2e/**/*.repor"
    path "tests/**/*.repor"
  }
}
"#;
        let config = parse_config(src).unwrap();
        assert_eq!(config.tests.paths.len(), 2);
    }

    #[test]
    fn unsupported_version_is_error() {
        let src = r#"
reportage {
  config {
    version 2
  }
  tests {
    path "e2e/**/*.repor"
  }
}
"#;
        let err = parse_config(src).unwrap_err();
        assert!(matches!(err, ConfigError::UnsupportedVersion(2)));
    }

    #[test]
    fn dot_segment_in_path_is_error() {
        let src = r#"
reportage {
  config {
    version 1
  }
  tests {
    path "./e2e/**/*.repor"
  }
}
"#;
        let err = parse_config(src).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidPath(_)));
    }

    #[test]
    fn double_dot_segment_in_path_is_error() {
        let src = r#"
reportage {
  config {
    version 1
  }
  tests {
    path "../e2e/**/*.repor"
  }
}
"#;
        let err = parse_config(src).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidPath(_)));
    }

    #[test]
    fn absolute_path_is_error() {
        let src = r#"
reportage {
  config {
    version 1
  }
  tests {
    path "/tmp/**/*.repor"
  }
}
"#;
        let err = parse_config(src).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidPath(_)));
    }

    #[test]
    fn missing_reportage_root_is_error() {
        let src = r#"
config {
  version 1
}
"#;
        let err = parse_config(src).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidStructure(_)));
    }

    // --- commands ---

    #[test]
    fn minimal_config_without_commands_has_empty_commands() {
        let src = r#"
reportage {
  config {
    version 1
  }
  tests {
    path "e2e/**/*.repor"
  }
}
"#;
        let config = parse_config(src).unwrap();
        assert!(config.commands.commands.is_empty());
    }

    #[test]
    fn one_command_is_parsed() {
        let src = r#"
reportage {
  config {
    version 1
  }
  commands {
    command "myapp" {
      exec "target/debug/myapp"
    }
  }
  tests {
    path "e2e/**/*.repor"
  }
}
"#;
        let config = parse_config(src).unwrap();
        assert_eq!(config.commands.commands.len(), 1);
        assert_eq!(config.commands.commands[0].id, "myapp");
        assert_eq!(config.commands.commands[0].exec, "target/debug/myapp");
    }

    #[test]
    fn multiple_commands_are_parsed() {
        let src = r#"
reportage {
  config {
    version 1
  }
  commands {
    command "myapp" {
      exec "target/debug/myapp"
    }
    command "other" {
      exec "target/debug/other"
    }
  }
  tests {
    path "e2e/**/*.repor"
  }
}
"#;
        let config = parse_config(src).unwrap();
        assert_eq!(config.commands.commands.len(), 2);
    }

    #[test]
    fn duplicate_command_id_is_error() {
        let src = r#"
reportage {
  config {
    version 1
  }
  commands {
    command "myapp" {
      exec "target/debug/myapp"
    }
    command "myapp" {
      exec "target/debug/other"
    }
  }
  tests {
    path "e2e/**/*.repor"
  }
}
"#;
        let err = parse_config(src).unwrap_err();
        assert!(matches!(err, ConfigError::DuplicateCommandId(id) if id == "myapp"));
    }

    #[test]
    fn missing_exec_is_error() {
        let src = r#"
reportage {
  config {
    version 1
  }
  commands {
    command "myapp" {
    }
  }
  tests {
    path "e2e/**/*.repor"
  }
}
"#;
        let err = parse_config(src).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidStructure(_)));
    }

    #[test]
    fn multiple_exec_is_error() {
        let src = r#"
reportage {
  config {
    version 1
  }
  commands {
    command "myapp" {
      exec "target/debug/myapp"
      exec "target/debug/other"
    }
  }
  tests {
    path "e2e/**/*.repor"
  }
}
"#;
        let err = parse_config(src).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidStructure(_)));
    }

    #[test]
    fn non_string_command_id_is_error() {
        let src = r#"
reportage {
  config {
    version 1
  }
  commands {
    command 123 {
      exec "target/debug/myapp"
    }
  }
  tests {
    path "e2e/**/*.repor"
  }
}
"#;
        let err = parse_config(src).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidStructure(_)));
    }

    #[test]
    fn non_string_exec_is_error() {
        let src = r#"
reportage {
  config {
    version 1
  }
  commands {
    command "myapp" {
      exec 123
    }
  }
  tests {
    path "e2e/**/*.repor"
  }
}
"#;
        let err = parse_config(src).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidStructure(_)));
    }

    #[test]
    fn empty_command_id_is_error() {
        let src = r#"
reportage {
  config {
    version 1
  }
  commands {
    command "" {
      exec "target/debug/myapp"
    }
  }
  tests {
    path "e2e/**/*.repor"
  }
}
"#;
        let err = parse_config(src).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidCommandId(_)));
    }

    #[test]
    fn command_id_with_slash_is_error() {
        let src = r#"
reportage {
  config {
    version 1
  }
  commands {
    command "bin/myapp" {
      exec "target/debug/myapp"
    }
  }
  tests {
    path "e2e/**/*.repor"
  }
}
"#;
        let err = parse_config(src).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidCommandId(_)));
    }

    #[test]
    fn command_id_dot_is_error() {
        let src = r#"
reportage {
  config {
    version 1
  }
  commands {
    command "." {
      exec "target/debug/myapp"
    }
  }
  tests {
    path "e2e/**/*.repor"
  }
}
"#;
        let err = parse_config(src).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidCommandId(_)));
    }

    #[test]
    fn command_id_dot_dot_is_error() {
        let src = r#"
reportage {
  config {
    version 1
  }
  commands {
    command ".." {
      exec "target/debug/myapp"
    }
  }
  tests {
    path "e2e/**/*.repor"
  }
}
"#;
        let err = parse_config(src).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidCommandId(_)));
    }

    #[test]
    fn command_id_with_nul_is_error() {
        // `\u{0000}` here is literal source text (not a Rust escape), left for the KDL parser
        // to interpret as its own unicode escape, producing an actual NUL byte in the parsed
        // command id.
        let src = "reportage {\n  config {\n    version 1\n  }\n  commands {\n    command \"tool\\u{0000}name\" {\n      exec \"target/debug/myapp\"\n    }\n  }\n  tests {\n    path \"e2e/**/*.repor\"\n  }\n}\n";
        let err = parse_config(src).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidCommandId(_)));
    }

    #[test]
    fn absolute_exec_path_is_error() {
        let src = r#"
reportage {
  config {
    version 1
  }
  commands {
    command "myapp" {
      exec "/usr/bin/myapp"
    }
  }
  tests {
    path "e2e/**/*.repor"
  }
}
"#;
        let err = parse_config(src).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidPath(_)));
    }

    #[test]
    fn dot_segment_exec_path_is_error() {
        let src = r#"
reportage {
  config {
    version 1
  }
  commands {
    command "myapp" {
      exec "./target/debug/myapp"
    }
  }
  tests {
    path "e2e/**/*.repor"
  }
}
"#;
        let err = parse_config(src).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidPath(_)));
    }

    #[test]
    fn unknown_node_in_commands_block_is_error() {
        let src = r#"
reportage {
  config {
    version 1
  }
  commands {
    typo "myapp" {
      exec "target/debug/myapp"
    }
  }
  tests {
    path "e2e/**/*.repor"
  }
}
"#;
        let err = parse_config(src).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidStructure(_)));
    }

    #[test]
    fn unknown_node_in_command_block_is_error() {
        let src = r#"
reportage {
  config {
    version 1
  }
  commands {
    command "myapp" {
      exec "target/debug/myapp"
      typo "oops"
    }
  }
  tests {
    path "e2e/**/*.repor"
  }
}
"#;
        let err = parse_config(src).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidStructure(_)));
    }
}
