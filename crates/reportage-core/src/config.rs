use kdl::KdlDocument;

#[derive(Debug)]
pub struct ReportageConfig {
    pub tests: TestsConfig,
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
    let tests = parse_tests(children)?;

    Ok(ReportageConfig { tests })
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
}
