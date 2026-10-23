# rukernel-compat

rukernel-compat is the differential test harness of [rukernel](https://github.com/tamnd/rukernel). It runs the same tests on rukernel and on Linux 7.3 in identical virtual machines. It compares the results and reports each difference.

The test oracle is a real Linux kernel. It is not a man page and it is not a standard. If Linux and rukernel give different results, rukernel has a bug. The only exceptions are the entries in the register, and each entry gives its reason.

The project is at milestone R0. The harness CLI, `rkc`, examines the data files and runs L1. The suite adapters come with the milestones.

## The compat levels

The harness reports five levels. [spec/18-rukernel-compat.md](https://github.com/tamnd/rukernel/blob/main/spec/18-rukernel-compat.md) gives the details.

| Level | What it compares |
|---|---|
| L1 | The generated ABI: system call numbers, structure layouts, constants and ioctl numbers |
| L2 | The static floor, then the system call suites: gVisor, LTP syscalls and the core kselftest collections |
| L3 | The subsystem suites: fstests, blktests, LTP net and mm, kselftest net and bpf, and CRIU |
| L4 | Distribution and container images: boot, smoke tests and the services of each image |
| L5 | Real applications: games under Proton, databases, browsers, IDEs and builds |

## The references

| Reference | Role |
|---|---|
| Linux 7.3 | The primary reference. The newest release. |
| Linux 6.18 | The second reference. The newest longterm release. |

Each reference is in [`references/`](references/). The harness builds each reference from its tag with the fragments in [`configs/`](configs/).

## The register

A difference from Linux is a bug, unless it has an entry in the register. Each entry has an id, a reason and, for an open problem, an issue.

| File | Contents |
|---|---|
| [`register/exclude.toml`](register/exclude.toml) | Tests that the harness does not run |
| [`register/settled.toml`](register/settled.toml) | Differences that are correct, such as the compiler name in `/proc/version` |
| [`register/untrustworthy.toml`](register/untrustworthy.toml) | Tests that give different results on the same kernel |

## Use

You need `rustup`. The toolchain file installs the correct Rust version.

```
cargo run --bin rkc -- help
cargo run --bin rkc -- check
cargo run --bin rkc -- l1 --reference 7.3 --abi ../rukernel/abi/7.3
cargo xtask ci
```

`rkc check` examines the references and the register. An entry without an issue or a reason is an error.

`rkc l1` compares the generated ABI in a rukernel tree with the uAPI headers of the reference. It downloads the reference tarball to `~/.cache/rkc` (or to `RKC_CACHE`), checks its SHA-256 and runs `make headers_install` for each architecture. Then clang compiles one `_Static_assert` for each value of the generated ABI. The command shows each difference, and it exits with 1 if there is a difference. It needs `curl`, `tar`, GNU make and clang. On macOS, install GNU make as `gmake`. Use `--headers DIR` to give headers that you installed, and `--arch` to compare one architecture.

## Repository layout

| Path | Contents |
|---|---|
| `rkc/` | The harness CLI |
| `references/` | One file for each reference kernel |
| `configs/` | The configuration fragments of each reference |
| `register/` | The exclusions, the settled differences and the untrustworthy tests |
| `suites/` | One adapter for each test suite |
| `xtask/` | The build checks |

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md). Report security problems as [SECURITY.md](SECURITY.md) tells you.

## License

The Apache License 2.0. Refer to [LICENSE](LICENSE). Each test suite keeps its own license.
