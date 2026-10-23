# Contributing

This document tells you how to make a change. The design of the harness is in [spec/18-rukernel-compat.md](https://github.com/tamnd/rukernel/blob/main/spec/18-rukernel-compat.md) in tamnd/rukernel.

## Before you start

Each milestone of rukernel needs parts of this harness. The milestone tracking issues have the label `kind/milestone`. Select an item and write a comment on the issue before you start.

## Run the checks

```
cargo xtask ci
```

This command runs `cargo fmt`, the prose rules, `cargo clippy`, `cargo test`, `cargo doc` and `rkc check`.

## The rules for a change

**Do not hide a failure.** Do not delete a test to make a run pass. Add an entry to the register. Give the reason and an issue.

**The reference decides.** A test is correct if it passes on the reference kernel. If a test fails on the reference kernel, it is not a test of rukernel. Put it in `register/untrustworthy.toml` or fix the test.

**Each suite is pinned.** An adapter uses a fixed version of its suite, with a checksum. To move the version, make a commit that changes the version and the checksum.

**Both kernels run in identical machines.** The VM definition, the image and the kernel command line are the same, except for the kernel. A difference in the machine is a bug in the harness.

## Style

**Rust.** `cargo fmt` sets the layout. A comment tells why, not what.

**Prose.** This rule applies to documents, commit messages, issues and pull requests. Use plain English. Use short sentences. Use the active voice. Do not use em dashes or en dashes. Do not use horizontal rules. Do not break a sentence across lines. `cargo xtask style` examines the dashes and the rules.

## Commits

Write the commit subject in the imperative mood. Start with the part that you change, for example `ltp: pin the 20260915 release`. A maintainer merges with squash or rebase.
