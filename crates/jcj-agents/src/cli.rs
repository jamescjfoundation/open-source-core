// SPDX-License-Identifier: MIT OR Apache-2.0
//! The parts of a command line every app writes the same way (decision 0017,
//! `docs/command-lines-and-agent-tools.md`): the exit codes, what `--help` and `--version` are
//! called, and the arguments of `mcp` with the two flags that confine an agent's files.
//!
//! ```
//! use jcj_agents::cli::{Exit, McpArgs, Standard, standard};
//!
//! let args: Vec<String> = ["mcp", "--read-root", "."].map(String::from).to_vec();
//! assert_eq!(standard(&args), None);
//! let mcp = McpArgs::parse(&args[1..]).unwrap();
//! assert_eq!(mcp.read_root.as_deref(), Some(std::path::Path::new(".")));
//! assert_eq!(standard(&[]), Some(Standard::NoCommand));
//! assert_eq!(Exit::Differs.code(), 3);
//! ```

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use jcj_engine::file_access::AutomationRoots;

/// How a command line ended: its exit code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Exit {
    /// 0: it did what was asked. For a check: the check passed.
    Done,
    /// 1: it could not do what was asked: a file could not be read or written, a command
    /// refused, the program met an error.
    Failed,
    /// 2: the command line was wrong: no command, an unknown command or option, a missing or
    /// extra argument, parameters that are not JSON. Nothing was done.
    Usage,
    /// 3: it ran, and a check of the content did not pass: two texts differ, or a check found a
    /// problem that blocks. Nothing is wrong with the program or the command line.
    Differs,
}

impl Exit {
    /// The exit code.
    pub const fn code(self) -> u8 {
        match self {
            Exit::Done => 0,
            Exit::Failed => 1,
            Exit::Usage => 2,
            Exit::Differs => 3,
        }
    }
}

impl From<Exit> for ExitCode {
    fn from(exit: Exit) -> ExitCode {
        ExitCode::from(exit.code())
    }
}

/// What the first argument asks for, when it is one of the requests every command line answers
/// the same way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standard {
    /// `--help`, `-h` or `help`: print the usage on standard output; [`Exit::Done`].
    Help,
    /// `--version` or `-V`: print [`version_line`] on standard output; [`Exit::Done`].
    Version,
    /// No argument at all: print the usage on standard error; [`Exit::Usage`]. Nothing was
    /// asked, so nothing was done, and a caller that lost its arguments does not read success.
    NoCommand,
}

/// Which standard request `args` (the arguments after the program's name) make, if any.
pub fn standard(args: &[String]) -> Option<Standard> {
    match args.first().map(String::as_str) {
        None => Some(Standard::NoCommand),
        Some("--help" | "-h" | "help") => Some(Standard::Help),
        Some("--version" | "-V") => Some(Standard::Version),
        Some(_) => None,
    }
}

/// The line `--version` prints: the program's name and its version (`notes-cli 1.2.0`). An app
/// may print more lines after it.
pub fn version_line(program: &str, version: &str) -> String {
    format!("{program} {version}")
}

/// The arguments of `<program> mcp`: an optional file to open, and the folders the agent's
/// commands may read and write.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct McpArgs {
    /// The document or project to open, if one was named.
    pub file: Option<String>,
    /// `--read-root <folder>`: the only folder commands may read files in.
    pub read_root: Option<PathBuf>,
    /// `--write-root <folder>`: the only folder commands may write files in.
    pub write_root: Option<PathBuf>,
}

impl McpArgs {
    /// The usage line, after the program's name.
    pub const USAGE: &'static str = "mcp [<file>] [--read-root <folder>] [--write-root <folder>]";

    /// Read the arguments that follow `mcp`. The error is the text of a usage error
    /// ([`Exit::Usage`]): an unknown option, a flag without its folder or given twice, a second
    /// file.
    pub fn parse(args: &[String]) -> Result<McpArgs, String> {
        let mut out = McpArgs::default();
        let mut rest = args.iter();
        while let Some(arg) = rest.next() {
            let slot = match arg.as_str() {
                "--read-root" => &mut out.read_root,
                "--write-root" => &mut out.write_root,
                option if option.starts_with('-') && option.len() > 1 => return Err(format!("unknown option {option}")),
                file => {
                    if out.file.is_some() {
                        return Err(format!("mcp takes one file, and {file} is a second"));
                    }
                    out.file = Some(file.to_owned());
                    continue;
                }
            };
            if slot.is_some() {
                return Err(format!("{arg} was given twice"));
            }
            let folder = rest.next().filter(|f| !f.starts_with("--")).ok_or_else(|| format!("{arg} needs a folder"))?;
            *slot = Some(PathBuf::from(folder));
        }
        Ok(out)
    }

    /// The folders the flags name, for [`crate::AgentConfig::confined`]: `None` when neither
    /// flag was given (nothing is confined). A flag given alone grants only its own kind of
    /// access: with `--read-root` alone nothing can be written. The error, a folder that does
    /// not exist, is a usage error.
    pub fn roots(&self) -> Result<Option<Arc<AutomationRoots>>, String> {
        AutomationRoots::new(self.read_root.as_deref(), self.write_root.as_deref())
    }

    /// [`McpArgs::roots`], put in force for the whole process, for good: every file the
    /// engine's commands read or write from here on is checked, on every thread. With neither
    /// flag it says so on standard error (standard output carries the protocol), naming
    /// `program`. Open the file the person named before calling this: it is theirs, not the
    /// agent's.
    pub fn confine(&self, program: &str) -> Result<Option<Arc<AutomationRoots>>, String> {
        let roots = self.roots()?;
        match &roots {
            Some(r) => {
                jcj_engine::file_access::confine_process(r.clone());
            }
            None => eprintln!("{program} mcp: no --read-root or --write-root given, so commands can read and write any file you can"),
        }
        Ok(roots)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|a| (*a).to_owned()).collect()
    }

    #[test]
    fn exit_codes_tell_a_difference_from_a_failure() {
        assert_eq!([Exit::Done, Exit::Failed, Exit::Usage, Exit::Differs].map(Exit::code), [0, 1, 2, 3]);
        assert_eq!(format!("{:?}", ExitCode::from(Exit::Differs)), format!("{:?}", ExitCode::from(3)));
    }

    #[test]
    fn help_version_and_no_command_are_the_same_everywhere() {
        for help in ["--help", "-h", "help"] {
            assert_eq!(standard(&args(&[help])), Some(Standard::Help));
        }
        for version in ["--version", "-V"] {
            assert_eq!(standard(&args(&[version, "extra"])), Some(Standard::Version));
        }
        assert_eq!(standard(&[]), Some(Standard::NoCommand));
        assert_eq!(standard(&args(&["mcp"])), None);
        assert_eq!(standard(&args(&["-v"])), None, "-v is not the version");
        assert_eq!(version_line("notes-cli", "1.2.0"), "notes-cli 1.2.0");
    }

    #[test]
    fn mcp_takes_a_file_and_two_roots_in_any_order() {
        assert_eq!(McpArgs::parse(&[]), Ok(McpArgs::default()));
        let all = McpArgs::parse(&args(&["--write-root", "out", "book.proj", "--read-root", "in"])).unwrap();
        assert_eq!(all, McpArgs { file: Some("book.proj".into()), read_root: Some("in".into()), write_root: Some("out".into()) });
        for (bad, why) in [
            (vec!["--read-root"], "needs a folder"),
            (vec!["--read-root", "--write-root", "x"], "needs a folder"),
            (vec!["--read-root", "a", "--read-root", "b"], "twice"),
            (vec!["--automation-read-root", "a"], "unknown option"),
            (vec!["-x"], "unknown option"),
            (vec!["one", "two"], "a second"),
        ] {
            let e = McpArgs::parse(&args(&bad)).unwrap_err();
            assert!(e.contains(why), "{bad:?}: {e}");
        }
    }

    #[test]
    fn roots_are_folders_that_exist_and_each_grants_its_own_access() {
        assert!(McpArgs::default().roots().unwrap().is_none(), "no flag: nothing is confined");
        let here = std::env::temp_dir();
        let read_only = McpArgs { read_root: Some(here.clone()), ..McpArgs::default() }.roots().unwrap().unwrap();
        assert!(read_only.read_root().is_some() && read_only.write_root().is_none());
        let inside = here.join("jcj-agents-cli-test.txt");
        let inside = inside.to_string_lossy();
        jcj_engine::file_access::confine(Some(&read_only), || {
            assert!(jcj_engine::file_access::check_read(&inside).is_ok());
            assert!(jcj_engine::file_access::check_write(&inside).is_err(), "--read-root alone lets nothing be written");
        });
        let missing = McpArgs { write_root: Some(here.join("jcj-agents-no-such-folder")), ..McpArgs::default() };
        assert!(missing.roots().is_err());
    }
}
