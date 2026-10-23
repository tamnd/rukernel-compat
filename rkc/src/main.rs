//! `rkc`, the differential test harness of rukernel.
//!
//! The harness runs the same test on rukernel and on a reference Linux kernel in identical virtual
//! machines. It compares the results and reports each difference. The design is in
//! spec/18-rukernel-compat.md in tamnd/rukernel.

#![forbid(unsafe_code)]

mod data;

use std::fs;
use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "\
rkc, the differential test harness of rukernel

usage: rkc <command>

commands that work now:
  check     examine the references and the register for errors
  version   show the version of rkc
  help      show this text

commands that come with the milestones of tamnd/rukernel:
  l1        compare the generated ABI with the reference headers (R0)
  run       run a suite on rukernel and on the reference (R1)
  diff      compare two runs and show each difference (R1)
  report    write the compat levels and the coverage table (R2)";

/// The keys that each entry of each register file must have, per spec/18-rukernel-compat.md
/// section 18.5.
const REGISTERS: &[(&str, &str, &[&str])] = &[
    ("register/exclude.toml", "exclude", &["id", "test", "reason", "issue"]),
    ("register/settled.toml", "settled", &["id", "test", "linux", "rukernel", "reason"]),
    ("register/untrustworthy.toml", "untrustworthy", &["id", "test", "reason", "issue"]),
];

/// The keys that each reference file must have.
const REFERENCE_KEYS: &[&str] = &["version", "tag", "role", "source"];

fn main() -> ExitCode {
    let command = std::env::args().nth(1).unwrap_or_else(|| String::from("help"));
    match command.as_str() {
        "check" => match check(Path::new(".")) {
            Ok(count) => {
                println!("rkc check: ok, {count} files");
                ExitCode::SUCCESS
            }
            Err(errors) => {
                eprintln!("rkc check: {} problems\n{}", errors.len(), errors.join("\n"));
                ExitCode::FAILURE
            }
        },
        "version" | "--version" | "-V" => {
            println!("rkc {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        "help" | "--help" | "-h" => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        "l1" | "run" | "diff" | "report" => {
            eprintln!("rkc {command}: this command is not in this version. Refer to `rkc help`.");
            ExitCode::from(2)
        }
        _ => {
            eprintln!("rkc: unknown command `{command}`\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// Examines the data files under `root`. Gives the number of files, or each error.
fn check(root: &Path) -> Result<usize, Vec<String>> {
    let mut errors = Vec::new();
    let mut count = 0;

    let references = root.join("references");
    let mut entries: Vec<_> = match fs::read_dir(&references) {
        Ok(dir) => dir.filter_map(|e| e.ok()).map(|e| e.path()).collect(),
        Err(e) => return Err(vec![format!("{}: {e}", references.display())]),
    };
    entries.sort();
    let mut primary = 0;
    for path in entries.iter().filter(|p| p.extension().is_some_and(|e| e == "toml")) {
        let shown = path.strip_prefix(root).unwrap_or(path).display().to_string();
        count += 1;
        match read_tables(path, &shown) {
            Ok(tables) => {
                let top = &tables[0];
                errors.extend(top.require(&shown, REFERENCE_KEYS));
                if top.values.get("role").is_some_and(|r| r == "primary") {
                    primary += 1;
                }
            }
            Err(e) => errors.push(e),
        }
    }
    if primary != 1 {
        errors.push(format!("references/: {primary} references have role = \"primary\", need 1"));
    }

    for (file, table, keys) in REGISTERS {
        count += 1;
        let tables = match read_tables(&root.join(file), file) {
            Ok(tables) => tables,
            Err(e) => {
                errors.push(e);
                continue;
            }
        };
        let mut ids = std::collections::BTreeSet::new();
        for t in tables.iter().skip(1) {
            if t.name != *table {
                errors.push(format!("{file}:{}: [[{}]] is not [[{table}]]", t.line, t.name));
                continue;
            }
            errors.extend(t.require(file, keys));
            if let Some(id) = t.values.get("id")
                && !ids.insert(id.clone())
            {
                errors.push(format!("{file}:{}: the id `{id}` is used two times", t.line));
            }
        }
    }

    if errors.is_empty() { Ok(count) } else { Err(errors) }
}

fn read_tables(path: &Path, shown: &str) -> Result<Vec<data::Table>, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{shown}: {e}"))?;
    data::parse(shown, &text)
}

#[cfg(test)]
mod tests {
    use super::check;
    use std::path::Path;

    #[test]
    fn the_data_in_this_repository_is_correct() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        assert_eq!(check(&root).map_err(|e| e.join("\n")), Ok(5));
    }
}
