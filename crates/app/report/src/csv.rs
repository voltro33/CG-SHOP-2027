//! CSV the way Python's `csv` module reads and writes it: a header row,
//! fields quoted only when they hold a comma, a quote or a line break.

use std::collections::HashMap;
use std::path::Path;

/// A CSV file: its header and its rows as text.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Table {
    pub header: Vec<String>,
    pub rows: Vec<Vec<String>>,
    columns: HashMap<String, usize>,
}

/// One row, read by column name.
#[derive(Debug, Clone, Copy)]
pub struct Row<'a> {
    table: &'a Table,
    cells: &'a [String],
}

impl<'a> Row<'a> {
    /// The cell under `column`; empty if the column or the cell is missing,
    /// as `csv.DictReader` fills short rows.
    pub fn get(&self, column: &str) -> &'a str {
        self.table
            .columns
            .get(column)
            .and_then(|&index| self.cells.get(index))
            .map_or("", String::as_str)
    }

    /// The cell under `column` read as a `T`.
    pub fn parse<T: std::str::FromStr>(&self, column: &str) -> Result<T, String> {
        let text = self.get(column);

        text.parse()
            .map_err(|_| format!("column {column}: {text:?} is not valid"))
    }
}

impl Table {
    pub fn parse(text: &str) -> Self {
        let mut records = records(text).into_iter();
        let header = records.next().unwrap_or_default();
        let columns = header
            .iter()
            .enumerate()
            .map(|(index, name)| (name.clone(), index))
            .collect();

        Self {
            header,
            rows: records.collect(),
            columns,
        }
    }

    pub fn read(path: &Path) -> Result<Self, String> {
        let text =
            std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;

        Ok(Self::parse(&text))
    }

    pub fn rows(&self) -> impl Iterator<Item = Row<'_>> + '_ {
        self.rows.iter().map(|cells| Row { table: self, cells })
    }
}

/// Splits `text` into records of fields, quotes undone. Blank lines are
/// skipped, as `csv.reader` skips them.
fn records(text: &str) -> Vec<Vec<String>> {
    let mut records = Vec::new();
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        match (quoted, c) {
            (true, '"') if chars.peek() == Some(&'"') => {
                chars.next();
                field.push('"');
            }
            (true, '"') => quoted = false,
            (true, c) => field.push(c),
            (false, '"') => quoted = true,
            (false, ',') => fields.push(std::mem::take(&mut field)),
            (false, '\r') => {}
            (false, '\n') => end_record(&mut records, &mut fields, &mut field),
            (false, c) => field.push(c),
        }
    }
    end_record(&mut records, &mut fields, &mut field);

    records
}

fn end_record(records: &mut Vec<Vec<String>>, fields: &mut Vec<String>, field: &mut String) {
    if fields.is_empty() && field.is_empty() {
        return;
    }

    fields.push(std::mem::take(field));
    records.push(std::mem::take(fields));
}

/// One line of fields, quoted as `csv.QUOTE_MINIMAL` does, ended by `end`.
pub fn line(fields: &[String], end: &str) -> String {
    let quoted: Vec<String> = fields.iter().map(|field| quote(field)).collect();

    quoted.join(",") + end
}

fn quote(field: &str) -> String {
    match field.contains([',', '"', '\n', '\r']) {
        true => format!("\"{}\"", field.replace('"', "\"\"")),
        false => field.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_quoted_fields_and_crlf() {
        let table = Table::parse("a,b\r\n1,\"x, \"\"y\"\"\"\r\n\r\n2\r\n");
        let rows: Vec<Row> = table.rows().collect();

        assert_eq!(table.header, ["a", "b"]);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].get("b"), "x, \"y\"");
        assert_eq!(rows[1].get("b"), "");
        assert_eq!(rows[1].parse::<u32>("a").unwrap(), 2);
        assert!(rows[1].parse::<u32>("b").is_err());
    }

    #[test]
    fn quotes_only_when_needed() {
        let fields = ["plain".to_owned(), "a,b".to_owned(), "q\"".to_owned()];

        assert_eq!(line(&fields, "\n"), "plain,\"a,b\",\"q\"\"\"\n");
    }
}
