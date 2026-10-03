//! Windows caps a command line at 32 767 UTF-16 units; big loader classpaths
//! exceed it. For Java 9+ the `-cp <classpath>` pair is moved into a Java
//! `@argfile` (which never contains secrets). Java 8 has no argfiles.

use std::path::Path;

use crate::error::{CoreError, Result};
use crate::fsutil::write_atomic;

/// Conservative budget (the limit includes the program path and quoting).
pub const MAX_COMMAND_LINE: usize = 30_000;

/// Approximate length of the command line Windows will see.
pub fn estimated_len(program: &Path, args: &[String]) -> usize {
    program.as_os_str().len() + 3 + args.iter().map(|a| a.len() + 3).sum::<usize>()
}

/// Quotes one argument for a Java argfile: inside double quotes a backslash
/// escapes the next character, so backslashes and quotes are doubled/escaped.
pub fn argfile_quote(arg: &str) -> String {
    let mut s = String::with_capacity(arg.len() + 2);
    s.push('"');
    for c in arg.chars() {
        match c {
            '\\' => s.push_str("\\\\"),
            '"' => s.push_str("\\\""),
            '\n' => s.push_str("\\n"),
            c => s.push(c),
        }
    }
    s.push('"');
    s
}

/// Rewrites `args` in place if they are too long. Returns the argfile path
/// that must be deleted after the game exits, if one was written.
pub fn fit(
    program: &Path,
    args: &mut Vec<String>,
    java_major: u32,
    argfile: &Path,
) -> Result<Option<std::path::PathBuf>> {
    if estimated_len(program, args) <= MAX_COMMAND_LINE {
        return Ok(None);
    }
    if java_major < 9 {
        return Err(CoreError::CommandLineTooLong { major: java_major });
    }
    let Some(i) = args.iter().position(|a| a == "-cp" || a == "-classpath") else {
        return Err(CoreError::CommandLineTooLong { major: java_major });
    };
    if i + 1 >= args.len() {
        return Err(CoreError::CommandLineTooLong { major: java_major });
    }
    let cp = args.remove(i + 1);
    let content = format!("-cp\n{}\n", argfile_quote(&cp));
    write_atomic(argfile, content.as_bytes())?;
    args[i] = format!("@{}", argfile.display());
    if estimated_len(program, args) > MAX_COMMAND_LINE {
        return Err(CoreError::CommandLineTooLong { major: java_major });
    }
    Ok(Some(argfile.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting() {
        assert_eq!(argfile_quote(r"C:\a b\c.jar"), r#""C:\\a b\\c.jar""#);
        assert_eq!(argfile_quote(r#"x"y"#), r#""x\"y""#);
    }

    #[test]
    fn short_lines_untouched() {
        let mut args = vec!["-cp".into(), "a;b".into(), "Main".into()];
        let dir = tempfile::tempdir().unwrap();
        assert!(
            fit(Path::new("java"), &mut args, 21, &dir.path().join("a.args"))
                .unwrap()
                .is_none()
        );
        assert_eq!(args.len(), 3);
    }

    #[test]
    fn long_classpath_moves_to_argfile() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("cp.args");
        let cp = vec![r"C:\libs\some\long\path.jar"; 2000].join(";");
        let mut args = vec![
            "-Xmx2G".into(),
            "-cp".into(),
            cp.clone(),
            "Main".into(),
            "--accessToken".into(),
            "secret".into(),
        ];
        let written = fit(Path::new("java"), &mut args, 21, &file)
            .unwrap()
            .unwrap();
        assert_eq!(args[1], format!("@{}", file.display()));
        assert_eq!(args.len(), 5);
        let content = std::fs::read_to_string(written).unwrap();
        assert!(content.starts_with("-cp\n\"C:\\\\libs"));
        assert!(
            !content.contains("secret"),
            "argfile must never contain secrets"
        );

        let mut args8 = vec!["-cp".into(), cp, "Main".into()];
        assert_eq!(
            fit(Path::new("java"), &mut args8, 8, &file)
                .unwrap_err()
                .code(),
            "launch.commandTooLong"
        );
    }
}
