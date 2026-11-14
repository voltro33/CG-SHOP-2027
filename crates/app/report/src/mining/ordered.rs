//! A map that remembers the order keys first came in, as a Python dict.

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Ordered<V> {
    entries: Vec<(String, V)>,
    index: HashMap<String, usize>,
}

impl<V> Default for Ordered<V> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
        }
    }
}

impl<V> Ordered<V> {
    /// The value of `key`, made by `make` the first time.
    pub fn entry(&mut self, key: &str, make: impl FnOnce() -> V) -> &mut V {
        let position = match self.index.get(key) {
            Some(&position) => position,
            None => {
                self.index.insert(key.to_owned(), self.entries.len());
                self.entries.push((key.to_owned(), make()));
                self.entries.len() - 1
            }
        };

        &mut self.entries[position].1
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &V)> + '_ {
        self.entries.iter().map(|(key, value)| (key.as_str(), value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_first_seen_order() {
        let mut map: Ordered<u32> = Ordered::default();
        *map.entry("b", || 0) += 1;
        *map.entry("a", || 0) += 1;
        *map.entry("b", || 0) += 1;

        let pairs: Vec<(&str, &u32)> = map.iter().collect();
        assert_eq!(pairs, [("b", &2), ("a", &1)]);
    }
}
