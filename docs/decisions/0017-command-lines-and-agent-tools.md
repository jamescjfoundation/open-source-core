# 0017: One convention for command lines and agent tools

Date: 2026-11-26 · Status: accepted · Extends decision 0007 (agents)

## Context

Each app built on the core has a command-line program that also serves the app's commands to
agents. Three were written one after another and each chose for itself:

| | Wordsmith Script | Wordsmith Manuscript | Wordsmith Book |
| --- | --- | --- | --- |
| Exit codes | 1 failed, 2 usage | 1 any error, usage included; 2 a word check found a difference | 1 a check failed, words differ or a command refused; 2 usage or a file that cannot be opened |
| `mcp` arguments | `[--read-root] [--write-root]` | `<project>`, required | `[<project>]` |
| Flags that keep an agent to chosen folders | `--read-root`, `--write-root` | none | none |
| Tool names | the command's id (`scene_list`) | `manuscript_…` | `book_…` |
| `--version` | `--version`, `-V` | `--version`, `version` | none |
| No argument | usage, 0 | usage, 0 | usage, 2 |

So a script that reads the exit code of a word check means the opposite in two apps, only one
of the three can keep an agent to folders its user chose, and two apps each wrote the same
adapter from their own commands to the engine's, around a global.

## Decision

`docs/command-lines-and-agent-tools.md` is the rule. What it decides, and why:

- **Exit codes: 0 done, 1 failed, 2 usage, 3 a check of the content did not pass.** "The two
  texts differ" is an answer, not a failure, and a script must be able to tell it from "the
  check did not run". 1 and 2 keep the meaning most programs give them (the standard library's
  failure code is 1, and a wrong command line is 2 almost everywhere), so the new meaning takes
  a code nothing else uses. The check's answer goes to standard output, a failure's reason to
  standard error.
- **No argument is a usage error (2), with the usage on standard error.** A caller that lost its
  arguments must not read success. `--help` is the way to ask for the usage and get 0.
- **`--version` and `-V` print `<program> <version>`** as their first line, so a script can
  read it the same way from every app.
- **`mcp [<file>] [--read-root <folder>] [--write-root <folder>]`.** The two flags are the
  engine's own confinement of an agent's files, under the names the first app gave them. A flag
  alone grants only its kind of access; with neither, nothing is confined and the program says
  so on standard error. Unconfined stays the default because a patch release may not change
  what a started server can reach; an app may make a root required in a release of its own.
- **The file named on the command line is the person's, not the agent's**: it is opened before
  the roots hold. The roots are for the paths an agent names.
- **Every tool that is the app's own is named `<app>_<command>`.** An agent connected to several
  apps then sees whose tool each is, and two apps with a command of the same id do not collide.
  The general document tools keep one name in every app.
- **The adapter is the core's** (`jcj_agents::AppCommands`, `AgentServer::for_app`): an app
  whose commands run on its own state gives their descriptions and one function that runs a
  command by id. The server names the tools, lists them, checks the paths their parameters name
  against the roots and calls them. It serves no general document tools for such an app: they
  would work on an empty document the app does not have.
- **The shared parts are code** (`jcj_agents::cli`: `Exit`, `standard`, `version_line`,
  `McpArgs`), so the convention is not three more copies of prose. `apps/core-testbed` follows
  it, and its test runs the built program and reads its exit codes and both outputs.

Everything here is opt-in in the core: the server's defaults, `AgentConfig` and
`AgentServer::headless` are unchanged, so an app on an earlier core release behaves as before
until it moves.

## Consequences

Each of the three apps changes something users of its command line can see: an exit code, a
tool's name, what no argument does. Each app records its own change in its changelog and guide,
and holds its command line to the page with a test.

A parameter that names a file and is not declared by its command is not checked: the adapter
cannot know. A path is checked and then opened, so the roots keep an agent to the files it was
given; they are not a guard against other programs on the computer (the engine's own note).

## Revisit when

An app needs a fifth outcome a script must tell apart; the Model Context Protocol gains a way to
give a server its roots, which would replace the two flags; or an app serves both a document and
its own project to one agent and the names of the general tools get in the way.
