//! A small hand-rolled argument parser: positional words, `--flag value`
//! options and bare `--switch`es, each command naming the ones it knows.

use std::str::FromStr;

/// What one command line said.
#[derive(Debug, Default)]
pub struct Parsed {
    pub positional: Vec<String>,
    values: Vec<(String, String)>,
    switches: Vec<String>,
}

/// Splits `raw` into positionals, the `valued` options and the `switches`.
/// Anything else that starts with `-` is an error.
pub fn parse(
    mut raw: impl Iterator<Item = String>,
    valued: &[&str],
    switches: &[&str],
) -> Result<Parsed, String> {
    let mut parsed = Parsed::default();

    while let Some(arg) = raw.next() {
        if valued.contains(&arg.as_str()) {
            let value = raw.next().ok_or_else(|| format!("{arg} needs a value"))?;
            parsed.values.push((arg, value));
            continue;
        }

        if switches.contains(&arg.as_str()) {
            parsed.switches.push(arg);
            continue;
        }

        if arg.starts_with('-') && arg.len() > 1 {
            return Err(format!("unknown option {arg}"));
        }

        parsed.positional.push(arg);
    }

    Ok(parsed)
}

impl Parsed {
    /// The last value given for `name`.
    pub fn value(&self, name: &str) -> Option<&str> {
        self.values
            .iter()
            .rev()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// The value of `name` read as a `T`, if given.
    pub fn number<T: FromStr>(&self, name: &str) -> Result<Option<T>, String> {
        match self.value(name) {
            None => Ok(None),
            Some(text) => text
                .parse()
                .map(Some)
                .map_err(|_| format!("{name}: {text:?} is not a valid number")),
        }
    }

    pub fn switch(&self, name: &str) -> bool {
        self.switches.iter().any(|s| s == name)
    }
}

/// `"0,1,7"` as numbers.
pub fn list<T: FromStr>(name: &str, text: &str) -> Result<Vec<T>, String> {
    text.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| item.parse().map_err(|_| format!("{name}: {item:?} is not valid")))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(line: &str) -> impl Iterator<Item = String> + '_ {
        line.split_whitespace().map(str::to_owned)
    }

    #[test]
    fn splits_options_switches_and_words() {
        let parsed = parse(words("a.json --time 5 seg --verify"), &["--time"], &["--verify"]).unwrap();

        assert_eq!(parsed.positional, ["a.json", "seg"]);
        assert_eq!(parsed.number::<f64>("--time").unwrap(), Some(5.0));
        assert!(parsed.switch("--verify"));
        assert_eq!(parsed.value("--out"), None);
    }

    #[test]
    fn refuses_unknown_and_incomplete_options() {
        assert!(parse(words("--nope"), &[], &[]).is_err());
        assert!(parse(words("--time"), &["--time"], &[]).is_err());

        let parsed = parse(words("--time x"), &["--time"], &[]).unwrap();
        assert!(parsed.number::<f64>("--time").is_err());
    }

    #[test]
    fn reads_lists() {
        assert_eq!(list::<u32>("--rounds", "0, 1,7").unwrap(), [0, 1, 7]);
        assert!(list::<u32>("--rounds", "0,x").is_err());
    }
}
