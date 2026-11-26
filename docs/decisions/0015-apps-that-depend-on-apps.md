# 0015: Apps that depend on apps

Date: 2026-11-26 · Status: accepted · Qualifies decision 0002 (rule 3) and extends decision 0013

## Context

Decision 0002 says an app depends only on the core's `jcj-` crates, and decision 0013 covers only
changes between an app and the core. One app built on the core already depends on two crates of
another, by a git tag, because it reads the other's project files. The dependency had no rule: no
list of what the consumer may use, no check on the provider's side, no place for the consumer's
requests, and two core releases in the consumer's graph.

## Decision

An app may depend on crates of another app built on the core, under a contract.

- **The providing app owns the contract.** It is a document in the provider's repository that
  lists the supported surface (the modules, types and functions a consumer may use) and the
  versions of every file format the consumer reads or writes through it. Anything not listed may
  change without notice.
- **The contract is versioned by tags.** The provider tags the commits a consumer may pin, by a
  scheme the contract states, and never moves such a tag. A consumer pins a tag, never a branch
  or a bare commit. A breaking change to the listed surface or to a listed format takes a new
  contract version and says what consumers must do, as a core minor release does.
- **The provider's CI builds the consumer.** A check in the provider compiles and tests each
  consumer, or a test that uses exactly the listed surface, against the provider's change, so a
  break is seen before it lands.
- **A request goes as an issue in the provider's repository.** Anyone working in the consumer,
  agents included, may open it. The consumer keeps at most an index of links to those issues.
- **A consumer never edits the provider from its own change.** A change to the provider is its
  own change in the provider's repository, landed first (decision 0013); then a tag; then the
  consumer's pin.
- **Both pin the same core release.** A consumer's graph holds one core (decision 0011), so the
  provider tags on the core release the consumer uses, and the two move to a new core together,
  provider first.
- **Only layers that carry no interface.** The provider offers crates that do not depend on the
  shell, the engine or its own interface code, so a consumer can use them from a command line.
- **Sources.** cargo-deny allows git dependencies only from listed addresses (decision 0001). A
  provider's repository must be on the consumer's list. While that list is in the core-owned
  `deny.toml`, the core's file names the provider; an app-owned list of extra sources is the
  better home and is to be added.

Something a second app needs still goes to the core when it is general (decision 0013, question
3). This decision is for what is about the provider's own subject: its model and its file format.

## Why

A consumer that reads another app's files needs that app's own reader: a second reader would
drift from the format, and moving the format into the core would put one product's concepts in a
base meant for any project. A dependency between apps is the honest shape. What made it unsafe
was not the dependency but the missing contract: the provider could not see what it would break,
and the consumer's needs sat in a file the provider never read.

## Revisit when

A third app needs the same crates (the shared part may then deserve a repository of its own), or
a provider's contract grows to most of its public interface.
