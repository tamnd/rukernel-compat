# Changelog

This file records each release. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The version numbers follow [Semantic Versioning](https://semver.org/).

## Unreleased

### Added

- The release workflow publishes `rkc` to crates.io. The publish script obeys the crates.io rate limits.
- The release workflow adds its files to a release that already exists.
- `rkc l1` compares the generated ABI of rukernel with the uAPI headers of the reference. It builds the headers from the reference tarball and checks the SHA-256 of the tarball. It compares the system call numbers, the constants and the structure layouts of x86-64, arm64 and riscv64. A CI job runs it against the main branch of rukernel.

### Added

- The harness CLI, `rkc`, with the `check` command.
- The CI gate, the security workflows and the release workflow.
