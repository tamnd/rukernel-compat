//! A reader for the data files of the harness.
//!
//! The data files use a small part of TOML: tables (`[name]`), arrays of tables (`[[name]]`) and
//! `key = value` lines with one value on each line. The reader does not need a dependency and it
//! gives a line number with each error. A file that uses more of TOML is an error.

use std::collections::BTreeMap;

/// One table in a data file.
#[derive(Debug, Default)]
pub struct Table {
    /// The name of the table. The top level has an empty name.
    pub name: String,
    /// The line of the table header.
    pub line: usize,
    /// The keys and the values, with the quotes removed from strings.
    pub values: BTreeMap<String, String>,
}

impl Table {
    /// Gives an error for each key in `keys` that the table does not have.
    pub fn require(&self, file: &str, keys: &[&str]) -> Vec<String> {
        keys.iter()
            .filter(|k| self.values.get(**k).is_none_or(|v| v.is_empty()))
            .map(|k| format!("{file}:{}: [{}] has no value for `{k}`", self.line, self.name))
            .collect()
    }
}

/// Reads a data file into its tables. The first table is the top level.
pub fn parse(file: &str, text: &str) -> Result<Vec<Table>, String> {
    let mut tables = vec![Table::default()];
    for (n, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix("[[").and_then(|l| l.strip_suffix("]]")) {
            tables.push(Table { name: name.trim().into(), line: n + 1, ..Table::default() });
        } else if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            tables.push(Table { name: name.trim().into(), line: n + 1, ..Table::default() });
        } else if let Some((key, value)) = line.split_once('=') {
            let value = value.trim();
            let value = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')).unwrap_or(value);
            if let Some(table) = tables.last_mut() {
                table.values.insert(key.trim().into(), value.into());
            }
        } else {
            return Err(format!("{file}:{}: cannot read this line", n + 1));
        }
    }
    Ok(tables)
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn tables_and_values() {
        let text = "version = \"7.3\"\n\n[[exclude]]\nid = \"a\"\nreason = \"\"\n";
        let tables = parse("t.toml", text).expect("valid file");
        assert_eq!(tables.len(), 2);
        assert_eq!(tables[0].values["version"], "7.3");
        assert_eq!(tables[1].name, "exclude");
        assert_eq!(tables[1].require("t.toml", &["id", "reason", "issue"]).len(), 2);
    }

    #[test]
    fn bad_line() {
        assert!(parse("t.toml", "this is not toml\n").is_err());
    }
}
