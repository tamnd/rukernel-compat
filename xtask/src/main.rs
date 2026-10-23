//! The build checks of this repository.
//!
//! `cargo xtask style` examines the prose. `cargo xtask ci` runs all the checks of the CI gate.

#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const TEXT_EXTENSIONS: &[&str] = &["md", "rs", "toml", "yml", "yaml", "txt"];

type Result<T = ()> = std::result::Result<T, String>;

fn main() -> ExitCode {
    let result = match std::env::args().nth(1).unwrap_or_default().as_str() {
        "style" => check_style(),
        "ci" => ci(),
        _ => Err(String::from(
            "usage: cargo xtask <style | ci>\n\n\
             style  the prose has no em dashes, no en dashes and no horizontal rules\n\
             ci     style, fmt, clippy, test, doc and the data check",
        )),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().map(Path::to_path_buf).unwrap_or(manifest)
}

fn check_style() -> Result {
    let root = workspace_root();
    let mut files = Vec::new();
    walk(&root, &mut files)?;
    let mut errors = Vec::new();
    for file in files {
        let text = fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let shown = file.strip_prefix(&root).unwrap_or(&file).display().to_string();
        let markdown = file.extension().is_some_and(|e| e == "md");
        let mut in_code = false;
        for (n, line) in text.lines().enumerate() {
            if line.contains('\u{2014}') {
                errors.push(format!("{shown}:{}: em dash", n + 1));
            }
            if line.contains('\u{2013}') {
                errors.push(format!("{shown}:{}: en dash", n + 1));
            }
            if markdown {
                if line.trim_start().starts_with("```") {
                    in_code = !in_code;
                } else if !in_code && is_horizontal_rule(line) {
                    errors.push(format!("{shown}:{}: horizontal rule, use a heading", n + 1));
                }
            }
        }
    }
    if errors.is_empty() {
        println!("prose rules: ok");
        Ok(())
    } else {
        Err(format!("prose rules: {} problems\n{}", errors.len(), errors.join("\n")))
    }
}

fn is_horizontal_rule(line: &str) -> bool {
    let chars: Vec<char> = line.chars().filter(|c| !c.is_whitespace()).collect();
    chars.len() >= 3 && matches!(chars[0], '-' | '*' | '_') && chars.iter().all(|&c| c == chars[0])
}

fn walk(dir: &Path, files: &mut Vec<PathBuf>) -> Result {
    let entries = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name == "target" || name == ".git" || name == "LICENSE" {
            continue;
        }
        if path.is_dir() {
            walk(&path, files)?;
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| TEXT_EXTENSIONS.contains(&e))
        {
            files.push(path);
        }
    }
    Ok(())
}

fn ci() -> Result {
    cargo(&["fmt", "--all", "--check"])?;
    check_style()?;
    cargo(&["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"])?;
    cargo(&["test", "--workspace", "--no-fail-fast"])?;
    cargo(&["doc", "--workspace", "--no-deps"])?;
    cargo(&["run", "--quiet", "--bin", env!("HARNESS_BIN"), "--", "check"])?;
    println!("all checks passed");
    Ok(())
}

fn cargo(args: &[&str]) -> Result {
    println!("cargo {}", args.join(" "));
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| String::from("cargo"));
    let status = Command::new(cargo)
        .args(args)
        .current_dir(workspace_root())
        .status()
        .map_err(|e| format!("cannot run cargo: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("cargo {} failed", args.join(" "))) }
}

#[cfg(test)]
mod tests {
    use super::is_horizontal_rule;

    #[test]
    fn horizontal_rules() {
        assert!(is_horizontal_rule("---"));
        assert!(!is_horizontal_rule("|---|---|"));
    }
}
