//! The tiles of a polishing pass and the order `aimed` puts them in equal
//! Python's (`tools/golden/polish/export_tiles.py`). Python's offset and
//! shuffle are handed over, since the random numbers differ.

mod common;

use std::fs;

use common::{dense, fixtures, numbers, problem, read_routes};
use mow_polish::{aimed, longest, tiles_at};

#[test]
fn tiles_and_aimed_order_match_python() {
    let dir = fixtures().join("tiles");
    let mut uids: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| {
            e.unwrap()
                .path()
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    uids.sort();

    let mut orders = 0;
    for uid in &uids {
        let Some(problem) = problem(uid) else {
            println!("{uid}: instance file missing, skipped");
            continue;
        };

        let tours = dense(&read_routes(&fixtures().join(format!("windows/{uid}.tours"))));
        let text = fs::read_to_string(dir.join(format!("{uid}.txt"))).unwrap();

        for line in text.lines() {
            let parts: Vec<&str> = line.split('|').map(str::trim).collect();
            let head: Vec<i32> = numbers(parts[0]);
            let (side, offset, expected_longest) = (head[0], head[1], head[2] as usize);
            assert_eq!(longest(&tours), expected_longest, "{uid}");

            let tiles = tiles_at(&tours[expected_longest], side, offset);
            let corners: Vec<i32> = tiles.iter().flat_map(|t| [t.x0, t.y0]).collect();
            assert_eq!(corners, numbers::<i32>(parts[1]), "{uid} side {side}");

            let shuffled: Vec<_> = numbers::<usize>(parts[2]).iter().map(|&i| tiles[i]).collect();
            let order: Vec<usize> = aimed(&problem, &tours, expected_longest, &shuffled)
                .iter()
                .map(|tile| tiles.iter().position(|t| t == tile).unwrap())
                .collect();
            assert_eq!(
                order,
                numbers::<usize>(parts[3]),
                "{uid} side {side} offset {offset}"
            );

            orders += 1;
        }
    }

    println!("{orders} tilings and aimed orders as Python");
}
