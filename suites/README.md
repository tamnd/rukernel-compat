# Suites

Each directory here holds the adapter for one test suite. An adapter fetches a pinned version of the suite, builds it, runs it in the VM and writes the results in one format. The adapters come with the milestones of tamnd/rukernel.

| Suite | Milestone | What it tests |
|---|---|---|
| gVisor syscall tests | R1 | The system call surface, with expectations from Linux |
| LTP | R1 | System calls, memory, scheduling, networking and file systems |
| kselftest | R2 | The selftests of each Linux subsystem |
| fstests | R3 | File systems |
| blktests | R3 | The block layer |
| systemd integration tests | R5 | The interfaces that systemd uses |
| OCI runtime tests, CRIU zdtm | R6 | Containers and checkpoint and restore |
| liburing tests | R6 | io_uring |
| kvm-unit-tests | R6 | The KVM host |
| BPF corpus | R7 | The verifier, the JIT, BTF and CO-RE |
| syzkaller | R2 | Crashes, compared with the reference |
