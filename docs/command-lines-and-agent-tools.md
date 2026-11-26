# Command lines and agent tools

How the command-line program of every app built on the core behaves, and how each app's commands
are named as tools for agents. One convention, so a script or an agent that has learned one app
knows the next. Decision 0017 says why each point is as it is; this page is the rule.

It covers an app's command-line program (`<app>-cli`). The code for the parts every app writes
the same way is `jcj_agents::cli`; `apps/core-testbed` follows this page, and
`apps/core-testbed/tests/command_line.rs` shows how a test holds a program to it.

## The command line

| | Rule |
| --- | --- |
| A command | `<program> <command> [arguments]`. Results are written to standard output; what went wrong, to standard error as `<program>: <reason>` |
| `--help`, `-h`, `help` | The usage on standard output, exit code 0 |
| `--version`, `-V` | One line on standard output, `<program> <version>`, exit code 0. More lines may follow it |
| No argument | The usage on standard error, exit code 2. Nothing was asked, so nothing was done |
| An unknown command or option, a missing or extra argument, parameters that are not JSON | The reason and the usage on standard error, exit code 2. Nothing is done |
| `mcp` | `<program> mcp [<file>] [--read-root <folder>] [--write-root <folder>]`: [below](#mcp) |

A desktop program that is also its app's window opens the window when run with no argument. It
follows the rest.

One start of a desktop program is not its user's and is answered before the command line is
read: on Windows the updater starts a copy of the program with `--finish-update` as its first
argument, to put a downloaded update in place once the app has exited. A program that installs
updates makes the shell's call for it first thing in `main`
(`jcj_shell::desktop::Desktop::finish_update`), and returns the code it is given. That start
opens no window, runs no command and prints no usage.

## Exit codes

| Code | `jcj_agents::cli::Exit` | Means |
| --- | --- | --- |
| 0 | `Done` | It did what was asked. For a check: the check passed |
| 1 | `Failed` | It could not do what was asked: a file could not be read or written, a command refused, the program met an error |
| 2 | `Usage` | The command line was wrong. Nothing was done |
| 3 | `Differs` | It ran, and a check of the content did not pass: two texts differ word for word, or a check found a problem that blocks an export |

3 is not a failure. A script that runs a word-for-word check reads 0 as "the same", 3 as "they
differ" and 1 or 2 as "the check did not run". The check's answer (what differs, what blocks) is
a result, so it goes to standard output with code 3; a reason for code 1 or 2 goes to standard
error.

A command that writes files and is stopped by a check of the content (an export that a failed
check blocks) ends with 3 where the program can tell that the check is the reason, and with 1
otherwise.

## `mcp`

`<program> mcp` serves the app's commands to an agent as tools: the Model Context Protocol, one
JSON-RPC message a line, on standard input and output, until standard input ends (exit code 0).
Standard output carries the protocol and nothing else; every other line goes to standard error.

| Argument | Rule |
| --- | --- |
| `<file>` | The document or project to open first. One at most. An app whose tools can open or make one may be started without it; an app whose tools all need one says so as a usage error |
| `--read-root <folder>` | The only folder the agent's commands may read files in |
| `--write-root <folder>` | The only folder the agent's commands may write files in |

- Each flag takes an existing folder; a folder that does not exist is a usage error. A path is
  inside a root when, made absolute with its links followed, it lies under it.
- A flag given alone grants only its own kind of access: with `--read-root` alone the agent's
  commands can write nothing, and with `--write-root` alone they can read nothing.
- With neither flag nothing is confined, and the program says so in one line on standard error
  when it starts.
- The roots hold for the paths an agent names: the parameters of a tool that name a file or a
  folder, and what the general tools open and save. The `<file>` on the command line is the
  person's choice, not the agent's: it is opened before the roots are put in force, and a
  project opened that way is saved where it lives.
- `jcj_agents::cli::McpArgs` reads these arguments (`parse`), makes the roots (`roots`) and puts
  them in force for the whole process (`confine`); `AgentConfig::confined` gives them to the
  server.

## Tool names

- The server says who it is in `initialize`: the app's machine name (`serverInfo.name`), the
  name people read, its version, and a few lines of instructions for the agent.
- **Every tool that is the app's own is named `<app>_<command>`**: the app's short name in
  lower case, an underscore, and the command's id with every character but letters, digits and
  `-` turned into `_` (`jcj_agents::app_tool_name`). The short name is one word, the same one
  for every tool of the app. A command whose id already starts with the short name as its
  namespace is not prefixed twice: `notes.rename` under `notes` is `notes_rename`.
- The general document tools an app with a document serves (`execute`, `list_commands`, typing,
  inspecting, rendering, opening and saving) keep the names the server gives them, the same in
  every app.
- An app whose commands work on a document session exports the namespaces that are its own
  (`AgentConfig::export`) and names them with `AgentServer::tool_prefix`.
- An app whose commands work on its own state (a project) describes them with
  `jcj_agents::AppCommands` and serves them with `AgentServer::for_app`. Its tools are its own
  commands only: the general document tools are left out, because there is no document behind
  them. Each command says which of its parameters name files it reads and files it writes
  (`AppCommand::reads`, `AppCommand::writes`), and the server checks them against the roots
  before the command runs. A parameter that names a file and is not declared is not checked.
- A tool says whether it only reads (`readOnlyHint`), and its description gives the command's
  id and its parameters.

## Names this page does not decide

The commands an app's command line offers besides `mcp` are the app's own. Where two apps offer
the same thing they use the same word: `convert`, `verify`, `report`, `run <file> <command>
[<JSON>]` for any command by its id, and `commands` for the list of them.
