//! A file browser for one side of a comparison: walk the folders, then
//! choose a folder of solutions or one solution file.
//!
//! The first row chooses the folder being shown; `..` goes up; folders open;
//! a `*.solution.json` file is chosen as it is. Each folder shows how many
//! solution files it holds, so the runs are easy to tell apart.

use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Cell, Paragraph, Row, Table, TableState};
use ratatui::Frame;

use super::{moved, step_of, Nav, Task};

pub const SOLUTION: &str = ".solution.json";

/// Which side of the comparison is being chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    A,
    B,
}

impl Slot {
    pub fn letter(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    /// Choose the folder being shown.
    Here,
    Parent,
    Folder,
    Solution,
}

#[derive(Debug, Clone)]
struct Entry {
    name: String,
    path: PathBuf,
    kind: Kind,
    /// Solution files directly inside, for folders.
    solutions: usize,
}

#[derive(Debug)]
pub struct Picker {
    pub slot: Slot,
    pub dir: PathBuf,
    /// Paths under it are shown relative to it.
    root: PathBuf,
    /// Where `h` goes back to.
    home: PathBuf,
    entries: Vec<Entry>,
    table: TableState,
}

impl Picker {
    /// Opens on `dir`, or on `home` if `dir` cannot be read.
    pub fn open(slot: Slot, dir: &Path, root: &Path, home: &Path) -> Self {
        let mut picker = Self {
            slot,
            dir: dir.to_path_buf(),
            root: root.to_path_buf(),
            home: home.to_path_buf(),
            entries: Vec::new(),
            table: TableState::default().with_selected(Some(0)),
        };

        if picker.go(dir).is_err() {
            let _ = picker.go(home);
        }
        picker
    }

    /// Shows `dir`, the selection on its first row.
    fn go(&mut self, dir: &Path) -> Result<(), String> {
        let entries = list(dir)?;

        self.dir = dir.to_path_buf();
        self.entries = entries;
        self.table.select(Some(0));
        Ok(())
    }

    /// Goes up, keeping the folder just left selected.
    fn up(&mut self) -> Nav {
        let Some(parent) = self.dir.parent().map(Path::to_path_buf) else {
            return Nav::Say("this is the top".to_owned());
        };
        let left = self.dir.clone();

        if let Err(error) = self.go(&parent) {
            return Nav::Say(error);
        }
        let at = self.entries.iter().position(|e| e.path == left).unwrap_or(0);
        self.table.select(Some(at));
        Nav::Stay
    }

    pub fn key(&mut self, key: KeyEvent) -> Nav {
        if let Some(step) = step_of(key, 15) {
            let at = moved(self.table.selected().unwrap_or(0), step, self.entries.len());
            self.table.select(Some(at));
            return Nav::Stay;
        }

        let entry = self.table.selected().and_then(|i| self.entries.get(i)).cloned();
        match key.code {
            KeyCode::Enter => match entry {
                Some(entry) => self.enter(&entry),
                None => Nav::Stay,
            },
            KeyCode::Right | KeyCode::Char('l') => match entry {
                Some(entry) if entry.kind == Kind::Folder => self.enter(&entry),
                _ => Nav::Stay,
            },
            KeyCode::Left | KeyCode::Backspace => self.up(),
            KeyCode::Char('h') => {
                let home = self.home.clone();
                self.go(&home).map_or_else(Nav::Say, |()| Nav::Stay)
            }
            KeyCode::Esc | KeyCode::Char('q') => Nav::Pop,
            _ => Nav::Stay,
        }
    }

    fn enter(&mut self, entry: &Entry) -> Nav {
        match entry.kind {
            Kind::Parent => self.up(),
            Kind::Folder => self.go(&entry.path).map_or_else(Nav::Say, |()| Nav::Stay),
            Kind::Solution => self.choose(entry.path.clone()),
            Kind::Here if entry.solutions == 0 => {
                Nav::Say("no *.solution.json files in this folder; open one that has some".to_owned())
            }
            Kind::Here => self.choose(self.dir.clone()),
        }
    }

    fn choose(&self, path: PathBuf) -> Nav {
        Nav::PopAndRun(Task::Pick {
            slot: self.slot,
            path,
        })
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let [head, body] = Layout::vertical([Constraint::Length(2), Constraint::Min(0)]).areas(area);

        let heading = Line::from(vec![
            ratatui::text::Span::styled(
                format!("Choose {}: ", self.slot.letter()),
                Style::new().add_modifier(Modifier::BOLD),
            ),
            ratatui::text::Span::styled(shown(&self.dir, &self.root), Style::new().fg(Color::Cyan)),
        ]);
        frame.render_widget(Paragraph::new(heading), head);

        let rows: Vec<Row> = self.entries.iter().map(row).collect();
        let table = Table::new(rows, [Constraint::Min(30), Constraint::Length(22)])
            .header(Row::new(["name", "holds"]).style(Style::new().add_modifier(Modifier::BOLD)))
            .row_highlight_style(Style::new().add_modifier(Modifier::REVERSED))
            .highlight_symbol("▶ ");
        frame.render_stateful_widget(table, body, &mut self.table);
    }
}

fn row(entry: &Entry) -> Row<'static> {
    let count = |n: usize| match n {
        0 => String::new(),
        1 => "1 solution".to_owned(),
        n => format!("{n} solutions"),
    };

    match entry.kind {
        Kind::Here => Row::new([
            Cell::from(match entry.solutions {
                0 => "▸ use this folder (it holds no solutions)".to_owned(),
                n => format!("▸ use this folder ({})", count(n)),
            })
            .style(Style::new().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Cell::from(""),
        ]),
        Kind::Parent => Row::new([Cell::from("..  (up)"), Cell::from("")]),
        Kind::Folder => Row::new([
            Cell::from(format!("{}/", entry.name)).style(Style::new().fg(Color::Cyan)),
            Cell::from(count(entry.solutions)).style(Style::new().fg(Color::Gray)),
        ]),
        Kind::Solution => Row::new([Cell::from(entry.name.clone()), Cell::from("solution file")]),
    }
}

/// The rows for `dir`: choose it, go up, its folders, its solution files.
fn list(dir: &Path) -> Result<Vec<Entry>, String> {
    let read = std::fs::read_dir(dir).map_err(|e| format!("cannot open {}: {e}", dir.display()))?;

    let mut folders = Vec::new();
    let mut files = Vec::new();
    for item in read.filter_map(Result::ok) {
        let path = item.path();
        let name = item.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }

        if path.is_dir() {
            folders.push(Entry {
                solutions: solutions_in(&path),
                name,
                path,
                kind: Kind::Folder,
            });
        } else if name.ends_with(SOLUTION) {
            files.push(Entry {
                name,
                path,
                kind: Kind::Solution,
                solutions: 0,
            });
        }
    }
    folders.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    files.sort_by(|a, b| a.name.cmp(&b.name));

    let mut entries = vec![Entry {
        name: String::new(),
        path: dir.to_path_buf(),
        kind: Kind::Here,
        solutions: files.len(),
    }];
    if let Some(parent) = dir.parent() {
        entries.push(Entry {
            name: "..".to_owned(),
            path: parent.to_path_buf(),
            kind: Kind::Parent,
            solutions: 0,
        });
    }
    entries.extend(folders);
    entries.extend(files);

    Ok(entries)
}

/// How many `*.solution.json` files sit directly in `dir`.
pub fn solutions_in(dir: &Path) -> usize {
    std::fs::read_dir(dir).map_or(0, |read| {
        read.filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().ends_with(SOLUTION))
            .count()
    })
}

/// `path` relative to `root` when it is under it, with forward slashes.
pub fn shown(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| match p.as_os_str().is_empty() {
            true => ".".to_owned(),
            false => p.to_string_lossy().replace('\\', "/"),
        })
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn press(picker: &mut Picker, code: KeyCode) -> Nav {
        picker.key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn tree(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mow-tui-picker-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (folder, files) in [("run/best", 2), ("other", 0)] {
            let at = dir.join(folder);
            std::fs::create_dir_all(&at).unwrap();
            for i in 0..files {
                std::fs::write(at.join(format!("u{i}{SOLUTION}")), "{}").unwrap();
            }
        }
        std::fs::write(dir.join("run/best/notes.txt"), "").unwrap();
        dir
    }

    #[test]
    fn folders_open_and_a_folder_of_solutions_is_chosen() {
        let dir = tree("open");
        let mut picker = Picker::open(Slot::B, &dir, &dir, &dir);

        // here, .., other/, run/
        assert_eq!(picker.entries.len(), 4);
        assert_eq!(picker.entries[3].name, "run");

        picker.table.select(Some(3));
        assert!(matches!(press(&mut picker, KeyCode::Enter), Nav::Stay));
        assert_eq!(picker.dir, dir.join("run"));
        assert_eq!(picker.entries[2].solutions, 2, "best/ holds two");

        picker.table.select(Some(2));
        press(&mut picker, KeyCode::Right);
        assert_eq!(
            picker.entries.len(),
            4,
            "here, .., two solution files; notes.txt hidden"
        );

        match press(&mut picker, KeyCode::Enter) {
            Nav::PopAndRun(Task::Pick { slot, path }) => {
                assert_eq!((slot, path), (Slot::B, dir.join("run/best")));
            }
            other => panic!("{other:?}"),
        }

        press(&mut picker, KeyCode::Left);
        assert_eq!(picker.dir, dir.join("run"));
        assert_eq!(picker.table.selected(), Some(2), "back on the folder just left");

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_folder_without_solutions_is_not_chosen_and_a_file_is() {
        let dir = tree("choose");
        let mut picker = Picker::open(Slot::A, &dir.join("other"), &dir, &dir);
        assert!(matches!(press(&mut picker, KeyCode::Enter), Nav::Say(_)));

        let mut picker = Picker::open(Slot::A, &dir.join("run/best"), &dir, &dir);
        picker.table.select(Some(2));
        match press(&mut picker, KeyCode::Enter) {
            Nav::PopAndRun(Task::Pick { path, .. }) => {
                assert_eq!(path, dir.join("run/best/u0.solution.json"))
            }
            other => panic!("{other:?}"),
        }

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn paths_show_relative_to_the_root() {
        let root = Path::new("/r");
        assert_eq!(shown(Path::new("/r/data/rust"), root), "data/rust");
        assert_eq!(shown(root, root), ".");
    }
}
