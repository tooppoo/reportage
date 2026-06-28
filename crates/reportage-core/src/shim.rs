use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum ShimError {
    EmptyCommandName,
    CommandNameContainsPathSeparator(String),
    ReservedCommandName(String),
    CommandNameContainsNul,
    RelativeProgramPath(PathBuf),
    NonUtf8ProgramPath(PathBuf),
    NonUtf8Argument(OsString),
    WriteError(std::io::Error),
}

impl std::fmt::Display for ShimError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShimError::EmptyCommandName => write!(f, "command name must not be empty"),
            ShimError::CommandNameContainsPathSeparator(name) => {
                write!(f, "command name '{name}' must not contain path separators ('/')")
            }
            ShimError::ReservedCommandName(name) => {
                write!(f, "command name '{name}' is reserved and cannot be used as a shim name")
            }
            ShimError::CommandNameContainsNul => {
                write!(f, "command name must not contain NUL bytes")
            }
            ShimError::RelativeProgramPath(path) => {
                write!(
                    f,
                    "program path '{}' must be absolute; relative paths are not supported",
                    path.display()
                )
            }
            ShimError::NonUtf8ProgramPath(path) => {
                write!(
                    f,
                    "program path '{}' contains non-UTF-8 bytes; non-UTF-8 program paths are not supported",
                    path.display()
                )
            }
            ShimError::NonUtf8Argument(arg) => {
                write!(
                    f,
                    "argument {arg:?} contains non-UTF-8 bytes; non-UTF-8 arguments are not supported"
                )
            }
            ShimError::WriteError(e) => write!(f, "failed to write shim: {e}"),
        }
    }
}

impl std::error::Error for ShimError {}

/// A validated POSIX file name used as a command shim name.
///
/// A command name is a single POSIX file name component, not a path. It must
/// not be empty, contain path separators, be `.` or `..`, or contain NUL bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandName(String);

impl CommandName {
    pub fn new(name: impl Into<String>) -> Result<Self, ShimError> {
        let name = name.into();
        if name.is_empty() {
            return Err(ShimError::EmptyCommandName);
        }
        if name.contains('\0') {
            return Err(ShimError::CommandNameContainsNul);
        }
        if name.contains('/') {
            return Err(ShimError::CommandNameContainsPathSeparator(name));
        }
        if name == "." || name == ".." {
            return Err(ShimError::ReservedCommandName(name));
        }
        Ok(Self(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An executable invocation: an absolute program path with optional fixed arguments.
///
/// Models the target of a command shim as an executable invocation rather than
/// a bare binary path. This allows the target to be a native executable, an
/// executable script, or an interpreter-plus-script invocation such as
/// `ruby tool.rb` (where `program` is the ruby interpreter path and `args`
/// contains the script path).
///
/// `program` must be an absolute path. Fixed `args` are prepended before the
/// caller-provided arguments (`"$@"`) in the generated wrapper.
///
/// Both `program` and `args` must be valid UTF-8; non-UTF-8 values are rejected
/// explicitly at construction time. See `TBD.md` for the deferred policy on
/// whether non-UTF-8 executable invocations may be supported later.
#[derive(Debug, Clone)]
pub struct ExecutableInvocation {
    pub program: PathBuf,
    pub args: Vec<OsString>,
}

impl ExecutableInvocation {
    pub fn new(program: PathBuf, args: Vec<OsString>) -> Result<Self, ShimError> {
        if !program.is_absolute() {
            return Err(ShimError::RelativeProgramPath(program));
        }
        if program.to_str().is_none() {
            return Err(ShimError::NonUtf8ProgramPath(program));
        }
        for arg in &args {
            if arg.to_str().is_none() {
                return Err(ShimError::NonUtf8Argument(arg.clone()));
            }
        }
        Ok(Self { program, args })
    }
}

/// A named POSIX command shim that delegates to an executable invocation.
///
/// `materialize` writes an executable POSIX shell wrapper at `dir/name` that
/// `exec`s the target invocation, forwarding all caller-provided arguments
/// after any fixed invocation arguments.
///
/// The generated wrapper uses single-quote shell escaping for the program path
/// and all fixed arguments, so paths containing spaces, single quotes, or other
/// shell-significant characters are handled safely.
///
/// If a file already exists at the destination, it is overwritten.
pub struct CommandShim {
    pub name: CommandName,
    pub target: ExecutableInvocation,
}

impl CommandShim {
    pub fn new(name: CommandName, target: ExecutableInvocation) -> Self {
        Self { name, target }
    }

    /// Write the POSIX wrapper script into `dir` as an executable file named
    /// after `self.name`.
    ///
    /// Returns a setup/runtime error if the file cannot be written or made
    /// executable; does not panic. If the destination already exists it is
    /// overwritten.
    pub fn materialize(&self, dir: &Path) -> Result<(), ShimError> {
        let content = self.wrapper_content();
        let dest = dir.join(self.name.as_str());
        std::fs::write(&dest, content).map_err(ShimError::WriteError)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755))
                .map_err(ShimError::WriteError)?;
        }
        Ok(())
    }

    fn wrapper_content(&self) -> String {
        // program and args are guaranteed UTF-8 by ExecutableInvocation::new.
        let program_str = self.target.program.to_str().unwrap();
        let mut parts = vec![shell_single_quote(program_str)];
        for arg in &self.target.args {
            parts.push(shell_single_quote(arg.to_str().unwrap()));
        }
        format!("#!/bin/sh\nexec {} \"$@\"\n", parts.join(" "))
    }
}

/// Wrap `s` in POSIX single quotes, escaping any embedded single quotes as `'\''`.
fn shell_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- CommandName validation ---

    #[test]
    fn valid_command_name_accepted() {
        assert!(CommandName::new("reportage").is_ok());
        assert!(CommandName::new("my-tool").is_ok());
        assert!(CommandName::new("tool_v2").is_ok());
    }

    #[test]
    fn empty_name_rejected() {
        assert!(matches!(
            CommandName::new(""),
            Err(ShimError::EmptyCommandName)
        ));
    }

    #[test]
    fn name_with_slash_rejected() {
        assert!(matches!(
            CommandName::new("bin/tool"),
            Err(ShimError::CommandNameContainsPathSeparator(_))
        ));
    }

    #[test]
    fn name_with_leading_slash_rejected() {
        assert!(matches!(
            CommandName::new("/tool"),
            Err(ShimError::CommandNameContainsPathSeparator(_))
        ));
    }

    #[test]
    fn dot_name_rejected() {
        assert!(matches!(
            CommandName::new("."),
            Err(ShimError::ReservedCommandName(_))
        ));
    }

    #[test]
    fn dot_dot_name_rejected() {
        assert!(matches!(
            CommandName::new(".."),
            Err(ShimError::ReservedCommandName(_))
        ));
    }

    #[test]
    fn name_with_nul_rejected() {
        assert!(matches!(
            CommandName::new("tool\0name"),
            Err(ShimError::CommandNameContainsNul)
        ));
    }

    // --- ExecutableInvocation validation ---

    #[test]
    fn absolute_path_with_no_args_accepted() {
        assert!(ExecutableInvocation::new(PathBuf::from("/usr/bin/true"), vec![]).is_ok());
    }

    #[test]
    fn relative_program_path_rejected() {
        assert!(matches!(
            ExecutableInvocation::new(PathBuf::from("relative/tool"), vec![]),
            Err(ShimError::RelativeProgramPath(_))
        ));
    }

    #[test]
    fn bare_relative_path_rejected() {
        assert!(matches!(
            ExecutableInvocation::new(PathBuf::from("tool"), vec![]),
            Err(ShimError::RelativeProgramPath(_))
        ));
    }

    #[test]
    #[cfg(unix)]
    fn non_utf8_program_path_rejected() {
        use std::os::unix::ffi::OsStringExt;
        let path = PathBuf::from(OsString::from_vec(b"/\xff/prog".to_vec()));
        assert!(matches!(
            ExecutableInvocation::new(path, vec![]),
            Err(ShimError::NonUtf8ProgramPath(_))
        ));
    }

    #[test]
    #[cfg(unix)]
    fn non_utf8_argument_rejected() {
        use std::os::unix::ffi::OsStringExt;
        let arg = OsString::from_vec(vec![0xff, 0xfe]);
        assert!(matches!(
            ExecutableInvocation::new(PathBuf::from("/usr/bin/ruby"), vec![arg]),
            Err(ShimError::NonUtf8Argument(_))
        ));
    }

    // --- shell_single_quote ---

    #[test]
    fn plain_string_is_single_quoted() {
        assert_eq!(shell_single_quote("/usr/bin/true"), "'/usr/bin/true'");
    }

    #[test]
    fn string_with_spaces_is_safely_quoted() {
        assert_eq!(
            shell_single_quote("/path with spaces/prog"),
            "'/path with spaces/prog'"
        );
    }

    #[test]
    fn string_with_single_quote_is_escaped() {
        assert_eq!(shell_single_quote("it's"), "'it'\\''s'");
    }

    #[test]
    fn string_with_dollar_sign_is_safe() {
        assert_eq!(shell_single_quote("$HOME"), "'$HOME'");
    }

    #[test]
    fn string_with_semicolon_is_safe() {
        assert_eq!(shell_single_quote("arg;rm -rf /"), "'arg;rm -rf /'");
    }

    #[test]
    fn string_with_backtick_is_safe() {
        assert_eq!(shell_single_quote("`cmd`"), "'`cmd`'");
    }

    // --- wrapper_content ---

    #[test]
    fn wrapper_content_no_fixed_args() {
        let name = CommandName::new("tool").unwrap();
        let invocation =
            ExecutableInvocation::new(PathBuf::from("/usr/bin/mytool"), vec![]).unwrap();
        let shim = CommandShim::new(name, invocation);
        assert_eq!(
            shim.wrapper_content(),
            "#!/bin/sh\nexec '/usr/bin/mytool' \"$@\"\n"
        );
    }

    #[test]
    fn wrapper_content_with_fixed_args() {
        let name = CommandName::new("tool").unwrap();
        let invocation = ExecutableInvocation::new(
            PathBuf::from("/usr/bin/ruby"),
            vec![OsString::from("/scripts/tool.rb")],
        )
        .unwrap();
        let shim = CommandShim::new(name, invocation);
        assert_eq!(
            shim.wrapper_content(),
            "#!/bin/sh\nexec '/usr/bin/ruby' '/scripts/tool.rb' \"$@\"\n"
        );
    }

    #[test]
    fn wrapper_content_path_with_spaces() {
        let name = CommandName::new("tool").unwrap();
        let invocation =
            ExecutableInvocation::new(PathBuf::from("/path with spaces/mytool"), vec![]).unwrap();
        let shim = CommandShim::new(name, invocation);
        assert_eq!(
            shim.wrapper_content(),
            "#!/bin/sh\nexec '/path with spaces/mytool' \"$@\"\n"
        );
    }

    #[test]
    fn wrapper_content_path_with_single_quote() {
        let name = CommandName::new("tool").unwrap();
        let invocation =
            ExecutableInvocation::new(PathBuf::from("/path'with'quotes/mytool"), vec![]).unwrap();
        let shim = CommandShim::new(name, invocation);
        assert_eq!(
            shim.wrapper_content(),
            "#!/bin/sh\nexec '/path'\\''with'\\''quotes/mytool' \"$@\"\n"
        );
    }

    #[test]
    fn wrapper_content_fixed_arg_with_dollar_sign() {
        let name = CommandName::new("tool").unwrap();
        let invocation = ExecutableInvocation::new(
            PathBuf::from("/usr/bin/prog"),
            vec![OsString::from("$VAR")],
        )
        .unwrap();
        let shim = CommandShim::new(name, invocation);
        assert_eq!(
            shim.wrapper_content(),
            "#!/bin/sh\nexec '/usr/bin/prog' '$VAR' \"$@\"\n"
        );
    }

    #[test]
    fn wrapper_content_fixed_arg_with_semicolon() {
        let name = CommandName::new("tool").unwrap();
        let invocation = ExecutableInvocation::new(
            PathBuf::from("/usr/bin/prog"),
            vec![OsString::from("a;b")],
        )
        .unwrap();
        let shim = CommandShim::new(name, invocation);
        assert_eq!(
            shim.wrapper_content(),
            "#!/bin/sh\nexec '/usr/bin/prog' 'a;b' \"$@\"\n"
        );
    }

    #[test]
    fn wrapper_content_fixed_arg_with_backtick() {
        let name = CommandName::new("tool").unwrap();
        let invocation = ExecutableInvocation::new(
            PathBuf::from("/usr/bin/prog"),
            vec![OsString::from("`date`")],
        )
        .unwrap();
        let shim = CommandShim::new(name, invocation);
        assert_eq!(
            shim.wrapper_content(),
            "#!/bin/sh\nexec '/usr/bin/prog' '`date`' \"$@\"\n"
        );
    }

    #[test]
    fn wrapper_content_fixed_arg_with_single_quote() {
        let name = CommandName::new("tool").unwrap();
        let invocation = ExecutableInvocation::new(
            PathBuf::from("/usr/bin/prog"),
            vec![OsString::from("it's")],
        )
        .unwrap();
        let shim = CommandShim::new(name, invocation);
        assert_eq!(
            shim.wrapper_content(),
            "#!/bin/sh\nexec '/usr/bin/prog' 'it'\\''s' \"$@\"\n"
        );
    }

    // --- materialize and execution integration ---

    #[test]
    #[cfg(unix)]
    fn materialized_shim_is_executable() {
        use std::os::unix::fs::PermissionsExt;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let name = CommandName::new("mytool").unwrap();
        let invocation =
            ExecutableInvocation::new(PathBuf::from("/usr/bin/true"), vec![]).unwrap();
        let shim = CommandShim::new(name, invocation);
        shim.materialize(dir.path()).unwrap();

        let dest = dir.path().join("mytool");
        let mode = std::fs::metadata(&dest).unwrap().permissions().mode();
        assert!(mode & 0o111 != 0, "shim must be executable");
    }

    #[test]
    #[cfg(unix)]
    fn wrapper_exits_with_target_exit_code() {
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let name = CommandName::new("myfail").unwrap();
        let false_path = which_bin("false");
        let invocation = ExecutableInvocation::new(false_path, vec![]).unwrap();
        let shim = CommandShim::new(name, invocation);
        shim.materialize(dir.path()).unwrap();

        let status = std::process::Command::new(dir.path().join("myfail"))
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(1));
    }

    #[test]
    #[cfg(unix)]
    fn wrapper_forwards_caller_arguments() {
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let name = CommandName::new("myecho").unwrap();
        let echo_path = which_bin("echo");
        let invocation = ExecutableInvocation::new(echo_path, vec![]).unwrap();
        let shim = CommandShim::new(name, invocation);
        shim.materialize(dir.path()).unwrap();

        let output = std::process::Command::new(dir.path().join("myecho"))
            .args(["hello", "world"])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "hello world");
    }

    #[test]
    #[cfg(unix)]
    fn fixed_args_come_before_caller_args() {
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let name = CommandName::new("myecho").unwrap();
        let echo_path = which_bin("echo");
        let invocation =
            ExecutableInvocation::new(echo_path, vec![OsString::from("fixed")]).unwrap();
        let shim = CommandShim::new(name, invocation);
        shim.materialize(dir.path()).unwrap();

        let output = std::process::Command::new(dir.path().join("myecho"))
            .arg("caller")
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "fixed caller"
        );
    }

    #[test]
    #[cfg(unix)]
    fn target_path_with_spaces_works() {
        use std::os::unix::fs::PermissionsExt;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();

        // Create a target in a directory whose name contains spaces.
        let spaced_dir = dir.path().join("path with spaces");
        std::fs::create_dir_all(&spaced_dir).unwrap();
        let target = spaced_dir.join("mytarget");
        std::fs::write(&target, "#!/bin/sh\necho ok\n").unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755)).unwrap();

        let shim_dir = dir.path().join("bin");
        std::fs::create_dir_all(&shim_dir).unwrap();

        let name = CommandName::new("mytool").unwrap();
        let invocation = ExecutableInvocation::new(target, vec![]).unwrap();
        let shim = CommandShim::new(name, invocation);
        shim.materialize(&shim_dir).unwrap();

        let output = std::process::Command::new(shim_dir.join("mytool"))
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "ok");
    }

    #[test]
    #[cfg(unix)]
    fn target_path_with_single_quote_works() {
        use std::os::unix::fs::PermissionsExt;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();

        // Create a target in a directory whose name contains a single quote.
        let quoted_dir = dir.path().join("path'with'quote");
        std::fs::create_dir_all(&quoted_dir).unwrap();
        let target = quoted_dir.join("mytarget");
        std::fs::write(&target, "#!/bin/sh\necho ok\n").unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755)).unwrap();

        let shim_dir = dir.path().join("bin");
        std::fs::create_dir_all(&shim_dir).unwrap();

        let name = CommandName::new("mytool").unwrap();
        let invocation = ExecutableInvocation::new(target, vec![]).unwrap();
        let shim = CommandShim::new(name, invocation);
        shim.materialize(&shim_dir).unwrap();

        let output = std::process::Command::new(shim_dir.join("mytool"))
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "ok");
    }

    #[test]
    #[cfg(unix)]
    fn fixed_arg_with_metacharacter_is_passed_literally() {
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let name = CommandName::new("myecho").unwrap();
        let echo_path = which_bin("echo");
        // A fixed arg with a shell metacharacter that must not be expanded.
        let invocation = ExecutableInvocation::new(
            echo_path,
            vec![OsString::from("$HOME"), OsString::from(";"), OsString::from("`date`")],
        )
        .unwrap();
        let shim = CommandShim::new(name, invocation);
        shim.materialize(dir.path()).unwrap();

        let output = std::process::Command::new(dir.path().join("myecho"))
            .output()
            .unwrap();
        // The metacharacters must be passed as literal strings, not interpreted.
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "$HOME ; `date`"
        );
    }

    #[test]
    #[cfg(unix)]
    fn materialize_overwrites_existing_file() {
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let name = CommandName::new("mytool").unwrap();

        // Write initial shim pointing to `true`.
        let true_path = which_bin("true");
        let invocation = ExecutableInvocation::new(true_path, vec![]).unwrap();
        let shim = CommandShim::new(name.clone(), invocation);
        shim.materialize(dir.path()).unwrap();

        // Overwrite with a shim pointing to `false`.
        let false_path = which_bin("false");
        let invocation2 = ExecutableInvocation::new(false_path, vec![]).unwrap();
        let shim2 = CommandShim::new(name, invocation2);
        shim2.materialize(dir.path()).unwrap();

        let status = std::process::Command::new(dir.path().join("mytool"))
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(1), "overwritten shim should point to false");
    }

    /// Resolve a standard binary by name using `which`.
    #[cfg(unix)]
    fn which_bin(name: &str) -> PathBuf {
        let output = std::process::Command::new("which")
            .arg(name)
            .output()
            .unwrap();
        PathBuf::from(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}
