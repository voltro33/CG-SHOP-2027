//! Writing SVG text: escaping, and paths split into chunks so no single
//! attribute grows without bound on a large lawn.

use std::borrow::Cow;
use std::fmt;
use std::io::{self, Write};

/// Subpaths per `<path>` element before a new one is started.
const CHUNK: usize = 4096;

/// `text` with the five XML special characters replaced, safe both as
/// element text and inside a double quoted attribute.
pub fn escape(text: &str) -> Cow<'_, str> {
    if !text.contains(['&', '<', '>', '"', '\'']) {
        return Cow::Borrowed(text);
    }

    let mut escaped = String::with_capacity(text.len() + 16);
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(c),
        }
    }

    Cow::Owned(escaped)
}

/// One logical path written as one or more `<path>` elements sharing the
/// same attributes. Nothing is written until the first subpath, so an empty
/// path leaves no element behind.
pub struct Paths<'w, W: Write> {
    out: &'w mut W,
    attributes: String,
    in_chunk: usize,
    total: usize,
}

impl<'w, W: Write> Paths<'w, W> {
    /// `attributes` is written as is into every element, e.g.
    /// `class="region" fill="#ccc"`.
    pub fn new(out: &'w mut W, attributes: String) -> Self {
        Self {
            out,
            attributes,
            in_chunk: 0,
            total: 0,
        }
    }

    /// Appends one subpath, e.g. `M0 0h2v2h-2z`.
    pub fn push(&mut self, subpath: fmt::Arguments<'_>) -> io::Result<()> {
        if self.in_chunk == CHUNK {
            self.close()?;
        }
        if self.in_chunk == 0 {
            write!(self.out, "<path {} d=\"", self.attributes)?;
        }

        self.out.write_fmt(subpath)?;
        self.in_chunk += 1;
        self.total += 1;

        Ok(())
    }

    /// Closes the last element. Returns how many subpaths were written.
    pub fn finish(mut self) -> io::Result<usize> {
        if self.in_chunk > 0 {
            self.close()?;
        }

        Ok(self.total)
    }

    fn close(&mut self) -> io::Result<()> {
        self.in_chunk = 0;
        self.out.write_all(b"\"/>\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_only_when_needed() {
        assert!(matches!(escape("plain"), Cow::Borrowed("plain")));
        assert_eq!(
            escape("a<b & 'c' \"d\">"),
            "a&lt;b &amp; &apos;c&apos; &quot;d&quot;&gt;"
        );
    }

    #[test]
    fn empty_paths_write_nothing() {
        let mut out = Vec::new();
        let paths = Paths::new(&mut out, "class=\"x\"".into());

        assert_eq!(paths.finish().unwrap(), 0);
        assert!(out.is_empty());
    }

    #[test]
    fn long_paths_are_split_into_chunks() {
        let mut out = Vec::new();
        let mut paths = Paths::new(&mut out, "class=\"x\"".into());
        for i in 0..CHUNK + 1 {
            paths.push(format_args!("M{i} 0")).unwrap();
        }

        assert_eq!(paths.finish().unwrap(), CHUNK + 1);
        let text = String::from_utf8(out).unwrap();
        assert_eq!(text.matches("<path ").count(), 2);
        assert_eq!(text.matches("\"/>").count(), 2);
    }
}
