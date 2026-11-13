//! Every stored best solution reads, checks and writes back byte for byte.

use std::path::Path;

use mow_io::SolutionFile;

#[test]
fn best_solutions_round_trip_byte_for_byte() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/solutions/best");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        println!("skipped: {} not found", dir.display());
        return;
    };

    let mut checked = 0;
    for entry in entries {
        let path = entry.unwrap().path();
        if !path.to_string_lossy().ends_with(".solution.json") {
            continue;
        }

        let original = std::fs::read_to_string(&path).unwrap();
        let file = SolutionFile::parse(&original).unwrap();
        let solution = file.to_solution().unwrap();

        assert_eq!(file.to_json().unwrap(), original, "{}", path.display());
        assert_eq!(solution.uid(), file.instance_uid);
        assert!(solution.objective() > 0, "{}", path.display());
        checked += 1;
    }

    println!("{checked} solutions round trip");
    assert!(checked > 0);
}
