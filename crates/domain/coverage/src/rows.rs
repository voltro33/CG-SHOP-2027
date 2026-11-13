//! Bit rows as `u64` word slices: bit `c % 64` of word `c / 64` is column `c`,
//! as in [`mow_core::BitGrid`].

const WORD: usize = u64::BITS as usize;

/// Word `i` of `row`, zero outside it.
fn word(row: &[u64], i: i64) -> u64 {
    usize::try_from(i)
        .ok()
        .and_then(|i| row.get(i))
        .copied()
        .unwrap_or(0)
}

/// `row |= row << s`: every set column `c` also sets `c + s`. In place, from
/// the high words down, so each word reads words not yet changed.
pub fn shift_or_up(row: &mut [u64], s: usize) {
    let (q, r) = ((s / WORD) as i64, (s % WORD) as u32);

    for w in (0..row.len()).rev() {
        let low = w as i64 - q;
        let mut bits = word(row, low) << r;
        if r > 0 {
            bits |= word(row, low - 1) >> (u64::BITS - r);
        }

        row[w] |= bits;
    }
}

/// Grows a row dilated by a window of `from` columns to a window of `to`:
/// afterwards column `c` is set if any of `c - to + 1 ..= c` was set before
/// the first widening. Doubling steps, so `log2(to)` shifts at most.
pub fn widen(row: &mut [u64], from: usize, to: usize) {
    let mut window = from.max(1);

    while window < to {
        let step = window.min(to - window);
        shift_or_up(row, step);
        window += step;
    }
}

/// `dst |= src` moved up by `shift` columns (down if negative): column `j`
/// of `dst` gets column `j - shift` of `src`. Columns outside `src` are zero.
pub fn or_shifted(dst: &mut [u64], src: &[u64], shift: i64) {
    for (w, out) in dst.iter_mut().enumerate() {
        let base = (w * WORD) as i64 - shift;
        let (q, r) = (base.div_euclid(WORD as i64), base.rem_euclid(WORD as i64) as u32);

        let mut bits = word(src, q) >> r;
        if r > 0 {
            bits |= word(src, q + 1) << (u64::BITS - r);
        }

        *out |= bits;
    }
}

/// Clears the padding columns `width..` of a row.
pub fn clear_tail(row: &mut [u64], width: u32) {
    let width = width as usize;
    let full = width / WORD;

    if let Some(last) = row.get_mut(full) {
        *last &= (1u64 << (width % WORD)) - 1;
    }
    for w in row.iter_mut().skip(full + 1) {
        *w = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn columns(row: &[u64]) -> Vec<usize> {
        (0..row.len() * WORD)
            .filter(|&c| row[c / WORD] >> (c % WORD) & 1 == 1)
            .collect()
    }

    fn row_of(cols: &[usize], words: usize) -> Vec<u64> {
        let mut row = vec![0; words];
        for &c in cols {
            row[c / WORD] |= 1 << (c % WORD);
        }
        row
    }

    #[test]
    fn widen_dilates_across_words() {
        let mut row = row_of(&[3, 100], 3);
        widen(&mut row, 1, 70);

        let expected: Vec<usize> = (3..=72).chain(100..=169).collect();
        assert_eq!(columns(&row), expected);
    }

    #[test]
    fn widen_from_a_wider_window_keeps_counting_from_the_start() {
        let mut row = row_of(&[0], 2);
        widen(&mut row, 1, 5);
        widen(&mut row, 5, 13);

        assert_eq!(columns(&row), (0..13).collect::<Vec<_>>());
    }

    #[test]
    fn or_shifted_moves_both_ways_and_clips() {
        let src = row_of(&[0, 63, 64, 127], 2);

        let mut up = vec![0; 3];
        or_shifted(&mut up, &src, 65);
        // 127 + 65 lands past the three words and is clipped
        assert_eq!(columns(&up), vec![65, 128, 129]);

        let mut down = vec![0; 2];
        or_shifted(&mut down, &src, -64);
        assert_eq!(columns(&down), vec![0, 63]);

        let mut same = vec![0; 2];
        or_shifted(&mut same, &src, 0);
        assert_eq!(same, src);
    }

    #[test]
    fn clear_tail_keeps_the_width() {
        let mut row = vec![u64::MAX; 3];
        clear_tail(&mut row, 70);
        assert_eq!(columns(&row), (0..70).collect::<Vec<_>>());

        let mut exact = vec![u64::MAX; 2];
        clear_tail(&mut exact, 128);
        assert_eq!(columns(&exact).len(), 128);
    }
}
