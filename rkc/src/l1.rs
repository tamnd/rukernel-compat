//! `rkc l1`, the ABI floor, per spec/18-rukernel-compat.md section 18.4.
//!
//! L1 compares the ABI that rk-abigen generated with the uAPI headers of the reference kernel. The
//! harness unpacks the reference tarball and runs `make headers_install` of the Linux tree for each
//! architecture. Thus the oracle is the output of the Linux build, not the output of rk-abigen.
//!
//! The checks do not use the method of rk-abigen. The system call numbers come from the text of
//! the generated `asm/unistd_*.h` headers. For the constants and the structure layouts, clang
//! compiles a file of `_Static_assert` lines against the headers. Each line asserts one value
//! from the generated ABI, so clang gives an error for each difference.

use crate::data::{self, Table};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

type Result<T = ()> = std::result::Result<T, String>;

/// An architecture of rukernel.
pub struct Arch {
    /// The name of the directory in the generated ABI, as `target_arch` gives it.
    pub name: &'static str,
    /// The `ARCH` of the Linux build.
    pub linux: &'static str,
    /// The target triple for clang.
    pub triple: &'static str,
    /// The headers with the native system call table, and for each header the ABIs in it. An
    /// empty list means the `native` list of the generated table.
    pub unistd: &'static [(&'static str, &'static [&'static str])],
}

/// The architectures of rukernel.
pub const ARCHES: &[Arch] = &[
    Arch {
        name: "x86_64",
        linux: "x86",
        triple: "x86_64-linux-gnu",
        unistd: &[("asm/unistd_64.h", &[]), ("asm/unistd_x32.h", &["common", "x32"])],
    },
    Arch {
        name: "aarch64",
        linux: "arm64",
        triple: "aarch64-linux-gnu",
        unistd: &[("asm/unistd_64.h", &[])],
    },
    Arch {
        name: "riscv64",
        linux: "riscv",
        triple: "riscv64-linux-gnu",
        unistd: &[("asm/unistd_64.h", &[])],
    },
];

/// The options of `rkc l1`.
pub struct Options {
    /// The version of the reference, for example `7.3`.
    pub reference: String,
    /// The directory of the generated ABI, `abi/<version>` in tamnd/rukernel.
    pub abi: PathBuf,
    /// A directory with the installed headers of each architecture, in `<linux arch>/include`.
    /// If it is not given, the harness makes the headers from the reference tarball.
    pub headers: Option<PathBuf>,
    /// The architectures to compare.
    pub arches: Vec<&'static Arch>,
}

impl Options {
    /// Reads the arguments after `l1`.
    pub fn parse(args: &[String]) -> Result<Self> {
        let mut reference = None;
        let mut abi = None;
        let mut headers = None;
        let mut arches = Vec::new();
        let mut it = args.iter();
        while let Some(arg) = it.next() {
            let mut value = || it.next().cloned().ok_or(format!("{arg} needs a value"));
            match arg.as_str() {
                "--reference" => reference = Some(value()?),
                "--abi" => abi = Some(PathBuf::from(value()?)),
                "--headers" => headers = Some(PathBuf::from(value()?)),
                "--arch" => {
                    let name = value()?;
                    let arch = ARCHES
                        .iter()
                        .find(|a| a.name == name)
                        .ok_or(format!("unknown architecture `{name}`"))?;
                    arches.push(arch);
                }
                other => return Err(format!("unknown argument `{other}`")),
            }
        }
        let reference = reference.ok_or("--reference is necessary, for example --reference 7.3")?;
        let abi = abi.unwrap_or_else(|| PathBuf::from(format!("../rukernel/abi/{reference}")));
        if arches.is_empty() {
            arches = ARCHES.iter().collect();
        }
        Ok(Self { reference, abi, headers, arches })
    }
}

/// The result of the comparison for one architecture.
pub struct ArchReport {
    /// The name of the architecture.
    pub arch: &'static str,
    /// The number of values that the comparison examined.
    pub checked: usize,
    /// Each difference.
    pub differences: Vec<String>,
}

/// Runs L1. `root` is the root of this repository.
pub fn run(root: &Path, options: &Options) -> Result<Vec<ArchReport>> {
    if !options.abi.is_dir() {
        return Err(format!(
            "{} is not a directory. Give the generated ABI of tamnd/rukernel with --abi",
            options.abi.display()
        ));
    }
    let headers = match &options.headers {
        Some(dir) => dir.clone(),
        None => install_headers(root, &options.reference, &options.arches)?,
    };
    let mut reports = Vec::new();
    for arch in &options.arches {
        let include = headers.join(arch.linux).join("include");
        let abi = options.abi.join(arch.name);
        reports.push(compare(arch, &abi, &include)?);
    }
    Ok(reports)
}

/// Compares the generated ABI of one architecture in `abi` with the headers in `include`.
pub fn compare(arch: &'static Arch, abi: &Path, include: &Path) -> Result<ArchReport> {
    let mut report = ArchReport { arch: arch.name, checked: 0, differences: Vec::new() };
    syscalls(arch, abi, include, &mut report)?;
    constants(arch, abi, include, &mut report)?;
    layouts(arch, abi, include, &mut report)?;
    Ok(report)
}

fn read(path: &Path) -> Result<Vec<Table>> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    data::parse(&path.display().to_string(), &text)
}

fn value<'a>(table: &'a Table, file: &Path, key: &str) -> Result<&'a str> {
    table
        .values
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| format!("{}:{}: no value for `{key}`", file.display(), table.line))
}

/// Reads a list of strings, such as `["common", "64"]`.
fn list(text: &str) -> Vec<String> {
    text.trim_matches(['[', ']'])
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Compares the system call numbers with each `asm/unistd_*.h` header of the architecture. Each
/// number in the header must be in the table, and each entry of the table with an ABI of the
/// header must be in the header.
fn syscalls(arch: &Arch, abi: &Path, include: &Path, report: &mut ArchReport) -> Result {
    let file = abi.join("syscalls.toml");
    let tables = read(&file)?;
    let native = list(value(&tables[0], &file, "native")?);
    for (header, abis) in arch.unistd {
        let abis: Vec<String> = if abis.is_empty() {
            native.clone()
        } else {
            abis.iter().map(|s| s.to_string()).collect()
        };
        let mut want = BTreeMap::new();
        for t in tables.iter().filter(|t| t.name == "syscall") {
            let abi = value(t, &file, "abi")?;
            if abis.iter().any(|a| a == abi) {
                let nr: u64 = value(t, &file, "nr")?
                    .parse()
                    .map_err(|_| format!("{}:{}: bad nr", file.display(), t.line))?;
                want.insert(value(t, &file, "name")?.to_string(), nr);
            }
        }
        let path = include.join(header);
        let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let got = unistd_numbers(&text);
        for (name, nr) in &want {
            report.checked += 1;
            match got.get(name) {
                Some(n) if n == nr => {}
                Some(n) => report.differences.push(format!(
                    "{header}: __NR_{name} is {n} in the reference and {nr} in the generated table"
                )),
                None => report.differences.push(format!(
                    "{header}: __NR_{name} is in the generated table and not in the reference"
                )),
            }
        }
        for (name, nr) in &got {
            if !want.contains_key(name) {
                report.differences.push(format!(
                    "{header}: __NR_{name} {nr} is in the reference and not in the generated table"
                ));
            }
        }
    }
    Ok(())
}

/// Reads the lines `#define __NR_name N` and `#define __NR_name (__X32_SYSCALL_BIT + N)`.
fn unistd_numbers(text: &str) -> BTreeMap<String, u64> {
    let mut numbers = BTreeMap::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("#define __NR_") else { continue };
        let Some((name, body)) = rest.split_once(char::is_whitespace) else { continue };
        let body = body.trim();
        let body = body
            .strip_prefix("(__X32_SYSCALL_BIT +")
            .and_then(|b| b.strip_suffix(')'))
            .unwrap_or(body)
            .trim();
        if let Ok(nr) = body.parse() {
            numbers.insert(name.to_string(), nr);
        }
    }
    numbers
}

/// The `_Static_assert` lines of one probe, with a description of each line.
struct Probe {
    source: String,
    lines: BTreeMap<usize, String>,
    next: usize,
}

impl Probe {
    fn new(includes: &[&str]) -> Self {
        let mut source = String::new();
        for h in includes {
            let _ = writeln!(source, "#include <{h}>");
        }
        Self { next: includes.len() + 1, source, lines: BTreeMap::new() }
    }

    fn assert(&mut self, condition: &str, what: String) {
        let _ = writeln!(self.source, "_Static_assert({condition}, \"\");");
        self.lines.insert(self.next, what);
        self.next += 1;
    }

    /// Compiles the probe. Gives the description of each line with an error, with the message of
    /// clang. An error outside the assert lines is an error of the probe.
    fn run(&self, arch: &Arch, include: &Path) -> Result<Vec<String>> {
        let mut child = Command::new(std::env::var("CLANG").unwrap_or_else(|_| "clang".into()))
            .arg(format!("--target={}", arch.triple))
            .arg("-D__EXPORTED_HEADERS__")
            .args(["-std=gnu11", "-ffreestanding", "-nostdlibinc", "-fsyntax-only"])
            .args(["-ferror-limit=0", "-fno-caret-diagnostics", "-fno-color-diagnostics"])
            .arg("-I")
            .arg(include)
            .args(["-x", "c", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("cannot run clang: {e}. Set CLANG to the path of clang"))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(self.source.as_bytes()).map_err(|e| e.to_string())?;
        }
        let out = child.wait_with_output().map_err(|e| e.to_string())?;
        let stderr = String::from_utf8_lossy(&out.stderr);
        let mut errors = Vec::new();
        let mut lines = stderr.lines().peekable();
        while let Some(line) = lines.next() {
            let Some((_, message)) = line.split_once(": error: ") else { continue };
            let n = line
                .strip_prefix("<stdin>:")
                .and_then(|r| r.split(':').next())
                .and_then(|n| n.parse::<usize>().ok());
            let Some(what) = n.and_then(|n| self.lines.get(&n)) else {
                return Err(format!("the probe for {} does not compile: {line}", arch.name));
            };
            // clang tells the values of a failed comparison in a note.
            let mut detail = message.trim_end_matches([':', ' ']).to_string();
            while let Some(note) = lines.next_if(|l| l.contains(": note: ")) {
                if let Some((_, n)) = note.split_once(": note: ") {
                    let _ = write!(detail, ", {n}");
                }
            }
            errors.push(format!("{what}: {detail}"));
        }
        if errors.is_empty() && !out.status.success() {
            return Err(format!("the probe for {} does not compile: {stderr}", arch.name));
        }
        Ok(errors)
    }
}

/// A C integer literal for a value of the generated ABI.
fn literal(text: &str) -> Result<String> {
    let v: i128 = text.parse().map_err(|_| format!("`{text}` is not an integer"))?;
    Ok(if v > i128::from(i64::MAX) { format!("{v}ULL") } else { format!("({v}LL)") })
}

/// The condition for the constant `name` with the value `v`. A negative `int` such as
/// `~FUTEX_PRIVATE_FLAG` has the type `u32` in the generated ABI, so the value is the bits of the
/// `int`. Thus a value from 2^31 to 2^32 - 1 also agrees with an `int` that has the same bits.
fn condition(name: &str, v: &str) -> Result<String> {
    let exact = format!("({name}) == {}", literal(v)?);
    let bits = v.parse::<i128>().is_ok_and(|v| (1 << 31..1 << 32).contains(&v));
    Ok(if bits {
        format!("{exact} || (sizeof({name}) == 4 && (unsigned int)({name}) == {v}U)")
    } else {
        exact
    })
}

/// Compares the constants. Each group has a header.
fn constants(arch: &Arch, abi: &Path, include: &Path, report: &mut ArchReport) -> Result {
    let file = abi.join("constants.toml");
    for group in read(&file)?.iter().skip(1) {
        let header = value(group, &file, "header")?;
        let mut probe = Probe::new(&[header]);
        for (name, v) in group.values.iter().filter(|(k, _)| *k != "header") {
            report.checked += 1;
            probe.assert(&condition(name, v)?, format!("{header}: {name} = {v}"));
        }
        report.differences.extend(probe.run(arch, include)?);
    }
    Ok(())
}

/// One structure of the generated ABI.
struct Struct {
    name: String,
    size: u64,
    align: u64,
    fields: Vec<(String, String, u64)>,
}

/// The Rust name that rk-abigen gives a C structure: `__kernel_fsid_t` is `KernelFsidT`.
fn rust_name(c: &str) -> String {
    c.split('_')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut chars = p.chars();
            chars
                .next()
                .map(|f| f.to_ascii_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        })
        .collect()
}

/// The size of a field type of the generated ABI: an integer, an array or a structure.
fn type_size(ty: &str, structs: &BTreeMap<String, u64>) -> Option<u64> {
    if let Some(inner) = ty.strip_prefix('[').and_then(|t| t.strip_suffix(']')) {
        let (elem, len) = inner.rsplit_once(';')?;
        return Some(type_size(elem.trim(), structs)? * len.trim().parse::<u64>().ok()?);
    }
    match ty {
        "u8" | "i8" => Some(1),
        "u16" | "i16" => Some(2),
        "u32" | "i32" => Some(4),
        "u64" | "i64" => Some(8),
        other => structs.get(other).copied(),
    }
}

/// The headers that declare the structures of the generated ABI. The generated file does not name
/// the header of each structure, so the probe includes all of them.
const LAYOUT_HEADERS: &[&str] = &[
    "asm/stat.h",
    "asm/statfs.h",
    "linux/stat.h",
    "linux/time_types.h",
    "linux/uio.h",
    "linux/utsname.h",
    "linux/resource.h",
    "asm/poll.h",
    "linux/sched.h",
    "linux/sched/types.h",
    "linux/eventpoll.h",
    "linux/futex.h",
    "linux/openat2.h",
    "linux/sysinfo.h",
    "asm/signal.h",
    "linux/capability.h",
    "asm/termbits.h",
    "asm/termios.h",
];

/// Compares the size, the alignment, and the offset and the size of each field of each structure.
fn layouts(arch: &Arch, abi: &Path, include: &Path, report: &mut ArchReport) -> Result {
    let file = abi.join("types.toml");
    let mut structs: Vec<Struct> = Vec::new();
    for t in read(&file)?.iter().skip(1) {
        let number = |key| {
            value(t, &file, key)?
                .parse::<u64>()
                .map_err(|_| format!("{}:{}: bad {key}", file.display(), t.line))
        };
        match t.name.as_str() {
            "struct" => structs.push(Struct {
                name: value(t, &file, "name")?.to_string(),
                size: number("size")?,
                align: number("align")?,
                fields: Vec::new(),
            }),
            "struct.field" => {
                let s = structs.last_mut().ok_or_else(|| {
                    format!("{}:{}: a field before a struct", file.display(), t.line)
                })?;
                let field = (
                    value(t, &file, "name")?.into(),
                    value(t, &file, "type")?.into(),
                    number("offset")?,
                );
                s.fields.push(field);
            }
            other => return Err(format!("{}:{}: unknown table [{other}]", file.display(), t.line)),
        }
    }
    let sizes: BTreeMap<String, u64> =
        structs.iter().map(|s| (rust_name(&s.name), s.size)).collect();

    // A name is a structure tag or the name of a typedef. A tag with no structure is an
    // incomplete type, so `sizeof` fails, and then the name is a typedef.
    let mut tags = Probe::new(LAYOUT_HEADERS);
    for s in &structs {
        tags.assert(&format!("sizeof(struct {}) > 0", s.name), s.name.clone());
    }
    let typedefs: Vec<String> = tags
        .run(arch, include)?
        .into_iter()
        .filter_map(|e| e.split_once(':').map(|(n, _)| n.to_string()))
        .collect();

    let mut probe = Probe::new(LAYOUT_HEADERS);
    for s in &structs {
        let ty =
            if typedefs.contains(&s.name) { s.name.clone() } else { format!("struct {}", s.name) };
        report.checked += 2;
        probe.assert(&format!("sizeof({ty}) == {}", s.size), format!("{ty}: size {}", s.size));
        probe.assert(&format!("_Alignof({ty}) == {}", s.align), format!("{ty}: align {}", s.align));
        for (field, fty, offset) in &s.fields {
            report.checked += 1;
            probe.assert(
                &format!("__builtin_offsetof({ty}, {field}) == {offset}"),
                format!("{ty}: offset of {field} {offset}"),
            );
            // A flexible array has no size.
            if let Some(size) = type_size(fty, &sizes).filter(|s| *s > 0) {
                report.checked += 1;
                probe.assert(
                    &format!("sizeof((({ty} *)0)->{field}) == {size}"),
                    format!("{ty}: size of {field} {size}"),
                );
            }
        }
    }
    report.differences.extend(probe.run(arch, include)?);
    Ok(())
}

/// The cache of the harness: `RKC_CACHE`, or `~/.cache/rkc`.
fn cache_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("RKC_CACHE") {
        return Ok(PathBuf::from(dir));
    }
    let home = std::env::var_os("HOME").ok_or("HOME is not set, set RKC_CACHE")?;
    Ok(PathBuf::from(home).join(".cache/rkc"))
}

fn command(cmd: &mut Command) -> Result<String> {
    let out = cmd.output().map_err(|e| format!("{cmd:?}: {e}"))?;
    if !out.status.success() {
        return Err(format!("{cmd:?}: {}\n{}", out.status, String::from_utf8_lossy(&out.stderr)));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Gives the SHA-256 of a file. Linux has `sha256sum` and macOS has `shasum`.
fn sha256(path: &Path) -> Result<String> {
    let out = command(Command::new("sha256sum").arg(path))
        .or_else(|_| command(Command::new("shasum").args(["-a", "256"]).arg(path)))?;
    out.split_whitespace().next().map(String::from).ok_or_else(|| format!("no hash for {path:?}"))
}

/// GNU make. macOS has GNU make 3.81 as `make`, and Linux needs 4.0 or later, so `gmake` is first.
fn gnu_make() -> &'static str {
    let has = |name| Command::new(name).arg("--version").output().is_ok_and(|o| o.status.success());
    if has("gmake") { "gmake" } else { "make" }
}

/// Unpacks the reference tarball and installs the uAPI headers of each architecture. Gives the
/// directory of the headers. The files stay in the cache, so a second run is fast.
fn install_headers(root: &Path, version: &str, arches: &[&Arch]) -> Result<PathBuf> {
    let file = root.join("references").join(format!("{version}.toml"));
    let tables = read(&file)?;
    let url = value(&tables[0], &file, "tarball")?;
    let want = value(&tables[0], &file, "sha256")?;

    let cache = cache_dir()?;
    fs::create_dir_all(&cache).map_err(|e| format!("{}: {e}", cache.display()))?;
    let tarball = cache.join(format!("linux-{version}.tar.xz"));
    if !tarball.exists() || sha256(&tarball)? != want {
        eprintln!("rkc l1: downloading {url}");
        let part = tarball.with_extension("part");
        command(
            Command::new("curl")
                .args(["--fail", "--location", "--silent", "--show-error", "--retry", "3"])
                .arg("--output")
                .arg(&part)
                .arg(url),
        )?;
        let got = sha256(&part)?;
        if got != want {
            return Err(format!("{url}: the SHA-256 is {got}, {} gives {want}", file.display()));
        }
        fs::rename(&part, &tarball).map_err(|e| format!("{}: {e}", tarball.display()))?;
    }

    let tree = cache.join(format!("linux-{version}"));
    let unpacked = cache.join(format!("linux-{version}.unpacked"));
    if !unpacked.exists() {
        if tree.exists() {
            fs::remove_dir_all(&tree).map_err(|e| format!("{}: {e}", tree.display()))?;
        }
        eprintln!("rkc l1: unpacking {}", tarball.display());
        command(Command::new("tar").arg("-xJf").arg(&tarball).arg("-C").arg(&cache))?;
        fs::write(&unpacked, want).map_err(|e| format!("{}: {e}", unpacked.display()))?;
    }

    let headers = cache.join(format!("headers-{version}"));
    for arch in arches {
        let dir = headers.join(arch.linux);
        let done = dir.join(".rkc-done");
        if done.exists() {
            continue;
        }
        eprintln!("rkc l1: make ARCH={} headers_install", arch.linux);
        command(
            Command::new(gnu_make())
                .arg("-C")
                .arg(&tree)
                .arg("-s")
                .arg(format!("ARCH={}", arch.linux))
                .arg(format!(
                    "O={}",
                    cache.join(format!("build-{version}")).join(arch.linux).display()
                ))
                .arg(format!("INSTALL_HDR_PATH={}", dir.display()))
                .arg("headers_install"),
        )?;
        fs::write(&done, "").map_err(|e| format!("{}: {e}", done.display()))?;
    }
    Ok(headers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unistd_lines() {
        let text = "#ifndef _ASM_UNISTD_64_H\n#define __NR_read 0\n#define __NR_write 1\n\
                    #define __NR_readv (__X32_SYSCALL_BIT + 515)\n#define __NR_syscalls nope\n";
        let n = unistd_numbers(text);
        assert_eq!(n.len(), 3);
        assert_eq!(n["read"], 0);
        assert_eq!(n["readv"], 515);
    }

    #[test]
    fn names_and_sizes() {
        assert_eq!(rust_name("__kernel_fsid_t"), "KernelFsidT");
        assert_eq!(rust_name("statx_timestamp"), "StatxTimestamp");
        let structs = BTreeMap::from([(String::from("KernelTimespec"), 16)]);
        assert_eq!(type_size("[u8; 65]", &structs), Some(65));
        assert_eq!(type_size("[KernelTimespec; 2]", &structs), Some(32));
        assert_eq!(type_size("Unknown", &structs), None);
        assert_eq!(literal("-100").as_deref(), Ok("(-100LL)"));
        assert_eq!(literal("18446744073709551615").as_deref(), Ok("18446744073709551615ULL"));
        assert_eq!(condition("A", "1").as_deref(), Ok("(A) == (1LL)"));
        assert!(
            condition("A", "4294967295")
                .is_ok_and(|c| c.contains("(unsigned int)(A) == 4294967295U"))
        );
    }

    /// A changed layout fails, per the exit of the L1 item. The test makes a small header and
    /// ABI, and it needs clang. Without clang it does nothing.
    #[test]
    fn a_changed_layout_fails() {
        let clang = std::env::var("CLANG").unwrap_or_else(|_| "clang".into());
        if Command::new(&clang).arg("--version").output().is_err() {
            eprintln!("no clang, the test does nothing");
            return;
        }
        let dir = std::env::temp_dir().join(format!("rkc-l1-test-{}", std::process::id()));
        let include = dir.join("include");
        let abi = dir.join("abi");
        for (path, text) in [
            (include.join("asm/unistd_64.h"), "#define __NR_read 0\n"),
            (include.join("asm/unistd_x32.h"), "#define __NR_read (__X32_SYSCALL_BIT + 0)\n"),
            (include.join("asm/errno.h"), "#define EPERM 1\n"),
            (include.join("asm/stat.h"), "struct stat { unsigned long st_dev; int st_mode; };\n"),
        ] {
            fs::create_dir_all(path.parent().unwrap_or(&dir)).expect("mkdir");
            fs::write(path, text).expect("write");
        }
        for h in LAYOUT_HEADERS.iter().filter(|h| **h != "asm/stat.h") {
            let path = include.join(h);
            fs::create_dir_all(path.parent().unwrap_or(&dir)).expect("mkdir");
            fs::write(path, "").expect("write");
        }
        let syscalls = "native = [\"common\", \"64\"]\n[[syscall]]\nnr = 0\nabi = \"common\"\nname = \"read\"\n";
        let constants = "[errno]\nheader = \"asm/errno.h\"\nEPERM = 1\n";
        let types = |offset: u64| {
            format!(
                "[[struct]]\nname = \"stat\"\nsize = 16\nalign = 8\n\
                 [[struct.field]]\nname = \"st_dev\"\ntype = \"u64\"\noffset = 0\n\
                 [[struct.field]]\nname = \"st_mode\"\ntype = \"i32\"\noffset = {offset}\n"
            )
        };
        let arch = &ARCHES[0];
        fs::create_dir_all(&abi).expect("mkdir");
        fs::write(abi.join("syscalls.toml"), syscalls).expect("write");
        fs::write(abi.join("constants.toml"), constants).expect("write");

        fs::write(abi.join("types.toml"), types(8)).expect("write");
        let good = compare(arch, &abi, &include).expect("compare");
        assert_eq!(good.differences, Vec::<String>::new());
        assert_eq!(good.checked, 2 + 1 + 2 + 4);

        fs::write(abi.join("types.toml"), types(12)).expect("write");
        let bad = compare(arch, &abi, &include).expect("compare");
        assert_eq!(bad.differences.len(), 1, "{:?}", bad.differences);
        assert!(bad.differences[0].contains("offset of st_mode 12"), "{:?}", bad.differences);
        let _ = fs::remove_dir_all(&dir);
    }
}
