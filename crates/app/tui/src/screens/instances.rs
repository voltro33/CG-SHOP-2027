//! Every instance in a table: sorted by a column, narrowed by k, by cutter
//! family and by a piece of the name. Enter opens one.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};
use ratatui::Frame;

use super::{moved, step_of, Ctx, Nav, Task};
use crate::data::{Instance, Lawn};

/// The columns a table can be sorted by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    Name,
    Cells,
    Gap,
    Points,
}

impl Sort {
    const ALL: [Self; 4] = [Self::Name, Self::Cells, Self::Gap, Self::Points];

    fn name(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Cells => "cells",
            Self::Gap => "gap",
            Self::Points => "points",
        }
    }

    fn next(self) -> Self {
        let i = Self::ALL.iter().position(|&s| s == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

#[derive(Debug)]
pub struct Instances {
    /// Indices into the lawn's instances, as filtered and sorted.
    pub rows: Vec<usize>,
    pub table: TableState,
    pub sort: Sort,
    /// Largest first.
    pub descending: bool,
    pub k: Option<usize>,
    pub family: Option<&'static str>,
    pub search: String,
    /// Keys go to the search box.
    pub typing: bool,
}

impl Instances {
    pub fn new(lawn: &Lawn) -> Self {
        let mut screen = Self {
            rows: Vec::new(),
            table: TableState::default().with_selected(Some(0)),
            sort: Sort::Gap,
            descending: true,
            k: None,
            family: None,
            search: String::new(),
            typing: false,
        };
        screen.refresh(lawn);

        screen
    }

    pub fn hints(&self) -> &'static str {
        match self.typing {
            true => "type a part of the name · Enter keep · Esc clear",
            false => "↑↓ choose · Enter open · s sort · d direction · k cutters · f family · / search · r refresh · Esc back",
        }
    }

    /// Filters and sorts again, keeping the selected instance if it stays.
    pub fn refresh(&mut self, lawn: &Lawn) {
        let selected = self.selected_uid(lawn).map(str::to_owned);

        let needle = self.search.to_lowercase();
        let mut rows: Vec<usize> = (0..lawn.instances.len())
            .filter(|&i| {
                let inst = &lawn.instances[i];
                self.k.map_or(true, |k| inst.k == k)
                    && self.family.map_or(true, |f| inst.family == f)
                    && inst.uid.to_lowercase().contains(&needle)
            })
            .collect();
        rows.sort_by(|&a, &b| order(self.sort, &lawn.instances[a], &lawn.instances[b]));
        if self.descending {
            rows.reverse();
        }
        self.rows = rows;

        let at = selected
            .and_then(|uid| self.rows.iter().position(|&i| lawn.instances[i].uid == uid))
            .unwrap_or(0);
        self.table.select(Some(at));
    }

    pub fn selected_uid<'a>(&self, lawn: &'a Lawn) -> Option<&'a str> {
        let row = *self.rows.get(self.table.selected()?)?;
        Some(lawn.instances[row].uid.as_str())
    }

    pub fn key(&mut self, key: KeyEvent, ctx: &Ctx) -> Nav {
        if self.typing {
            return self.type_key(key, &ctx.lawn);
        }
        if let Some(step) = step_of(key, 20) {
            let at = moved(self.table.selected().unwrap_or(0), step, self.rows.len());
            self.table.select(Some(at));
            return Nav::Stay;
        }

        match key.code {
            KeyCode::Enter => match self.selected_uid(&ctx.lawn) {
                Some(uid) => Nav::Run(Task::Detail(uid.to_owned())),
                None => Nav::Stay,
            },
            KeyCode::Char('s') => self.change(&ctx.lawn, |s| s.sort = s.sort.next()),
            KeyCode::Char('d') => self.change(&ctx.lawn, |s| s.descending = !s.descending),
            KeyCode::Char('k') => self.change(&ctx.lawn, |s| s.k = next_k(s.k)),
            KeyCode::Char('f') => {
                let families = families(&ctx.lawn);
                self.change(&ctx.lawn, |s| s.family = next_family(s.family, &families))
            }
            KeyCode::Char('/') => {
                self.typing = true;
                Nav::Stay
            }
            KeyCode::Char('r') => Nav::Run(Task::Reload),
            KeyCode::Esc | KeyCode::Backspace | KeyCode::Char('q') => Nav::Pop,
            _ => Nav::Stay,
        }
    }

    fn type_key(&mut self, key: KeyEvent, lawn: &Lawn) -> Nav {
        match key.code {
            KeyCode::Enter => self.typing = false,
            KeyCode::Esc => {
                self.typing = false;
                self.search.clear();
            }
            KeyCode::Backspace => {
                self.search.pop();
            }
            KeyCode::Char(c) => self.search.push(c),
            _ => return Nav::Stay,
        }

        self.refresh(lawn);
        Nav::Stay
    }

    fn change(&mut self, lawn: &Lawn, edit: impl FnOnce(&mut Self)) -> Nav {
        edit(self);
        self.refresh(lawn);
        Nav::Stay
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) {
        let [filters, body] = Layout::vertical([Constraint::Length(2), Constraint::Min(0)]).areas(area);

        frame.render_widget(self.filter_line(), filters);

        let rows: Vec<Row> = self.rows.iter().map(|&i| row(&ctx.lawn.instances[i])).collect();
        let table = Table::new(rows, WIDTHS)
            .header(Row::new(HEADER).style(Style::new().add_modifier(Modifier::BOLD)))
            .row_highlight_style(Style::new().add_modifier(Modifier::REVERSED))
            .highlight_symbol("▶ ");
        frame.render_stateful_widget(table, body, &mut self.table);
    }

    fn filter_line(&self) -> Paragraph<'static> {
        let k = self.k.map_or("all".to_owned(), |k| k.to_string());
        let family = self.family.unwrap_or("all");
        let direction = if self.descending { "↓" } else { "↑" };
        let search = match (self.typing, self.search.is_empty()) {
            (true, _) => format!("{}▏", self.search),
            (false, true) => "–".to_owned(),
            (false, false) => self.search.clone(),
        };

        let text = format!(
            "{} shown · sort {} {direction} · cutters {k} · family {family} · search {search}",
            self.rows.len(),
            self.sort.name()
        );
        Paragraph::new(text)
            .style(Style::new().fg(Color::Gray))
            .block(Block::new().borders(Borders::BOTTOM))
    }
}

const HEADER: [&str; 9] = [
    "instance", "k", "family", "cells", "bound", "best", "gap", "points", "found by",
];

const WIDTHS: [Constraint; 9] = [
    Constraint::Length(32),
    Constraint::Length(2),
    Constraint::Length(15),
    Constraint::Length(9),
    Constraint::Length(7),
    Constraint::Length(7),
    Constraint::Length(7),
    Constraint::Length(7),
    Constraint::Min(10),
];

fn row(inst: &Instance) -> Row<'static> {
    let best = inst.best.as_ref();

    Row::new([
        Cell::from(inst.uid.clone()),
        Cell::from(inst.k.to_string()),
        Cell::from(inst.family),
        Cell::from(inst.cells.to_string()),
        Cell::from(inst.lower_bound.to_string()),
        Cell::from(best.map_or("–".to_owned(), |b| b.objective.to_string())),
        Cell::from(inst.gap().map_or("–".to_owned(), |g| format!("{g:.1}%"))),
        Cell::from(format!("{:.3}", inst.points())),
        Cell::from(best.map_or(String::new(), |b| b.label.clone())),
    ])
}

fn order(sort: Sort, a: &Instance, b: &Instance) -> std::cmp::Ordering {
    let by_name = a.uid.cmp(&b.uid);

    match sort {
        Sort::Name => by_name,
        Sort::Cells => a.cells.cmp(&b.cells).then(by_name),
        Sort::Gap => a
            .gap()
            .unwrap_or(f64::INFINITY)
            .total_cmp(&b.gap().unwrap_or(f64::INFINITY))
            .then(by_name),
        Sort::Points => a.points().total_cmp(&b.points()).then(by_name),
    }
}

/// All, then 1, 2 and 3 cutters.
fn next_k(k: Option<usize>) -> Option<usize> {
    match k {
        None => Some(1),
        Some(k) if k < 3 => Some(k + 1),
        Some(_) => None,
    }
}

fn families(lawn: &Lawn) -> Vec<&'static str> {
    let mut all: Vec<&'static str> = lawn.instances.iter().map(|i| i.family).collect();
    all.sort_unstable();
    all.dedup();
    all
}

/// All, then each family in turn.
fn next_family(family: Option<&'static str>, families: &[&'static str]) -> Option<&'static str> {
    match family {
        None => families.first().copied(),
        Some(f) => {
            let i = families.iter().position(|&g| g == f)?;
            families.get(i + 1).copied()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::tests::instance;
    use crate::data::Paths;
    use crate::summary::Summary;
    use crossterm::event::KeyModifiers;

    fn ctx() -> Ctx {
        let lawn = Lawn {
            instances: vec![
                instance("alpha_1s3", 1, "square", Some(150)),
                instance("beta_2x5", 2, "X", Some(110)),
                instance("gamma_3x5", 3, "X", Some(200)),
                instance("delta_1s3", 1, "square", None),
            ],
            latest: None,
        };
        let summary = Summary::of(&lawn);
        let paths = Paths {
            root: ".".into(),
            data: ".".into(),
            instances: ".".into(),
        };

        Ctx { paths, lawn, summary }
    }

    fn press(screen: &mut Instances, ctx: &Ctx, code: KeyCode) -> Nav {
        screen.key(KeyEvent::new(code, KeyModifiers::NONE), ctx)
    }

    fn shown(screen: &Instances, ctx: &Ctx) -> Vec<String> {
        screen
            .rows
            .iter()
            .map(|&i| ctx.lawn.instances[i].uid.clone())
            .collect()
    }

    #[test]
    fn the_widest_gap_comes_first_and_missing_bests_lead() {
        let ctx = ctx();
        let screen = Instances::new(&ctx.lawn);

        assert_eq!(
            shown(&screen, &ctx),
            ["delta_1s3", "gamma_3x5", "alpha_1s3", "beta_2x5"]
        );
    }

    #[test]
    fn filters_narrow_and_search_types_into_the_box() {
        let ctx = ctx();
        let mut screen = Instances::new(&ctx.lawn);

        press(&mut screen, &ctx, KeyCode::Char('k'));
        assert_eq!(shown(&screen, &ctx), ["delta_1s3", "alpha_1s3"]);

        press(&mut screen, &ctx, KeyCode::Char('k'));
        press(&mut screen, &ctx, KeyCode::Char('k'));
        press(&mut screen, &ctx, KeyCode::Char('k'));
        press(&mut screen, &ctx, KeyCode::Char('f'));
        assert_eq!(screen.family, Some("X"));
        assert_eq!(shown(&screen, &ctx).len(), 2);

        press(&mut screen, &ctx, KeyCode::Char('/'));
        press(&mut screen, &ctx, KeyCode::Char('b'));
        press(&mut screen, &ctx, KeyCode::Char('e'));
        assert_eq!(shown(&screen, &ctx), ["beta_2x5"]);

        // while typing, k is a letter, not the cutter filter
        press(&mut screen, &ctx, KeyCode::Esc);
        assert!(screen.search.is_empty());
        assert_eq!(shown(&screen, &ctx).len(), 2);
    }

    #[test]
    fn enter_asks_for_the_selected_instance() {
        let ctx = ctx();
        let mut screen = Instances::new(&ctx.lawn);

        press(&mut screen, &ctx, KeyCode::Down);
        let nav = press(&mut screen, &ctx, KeyCode::Enter);

        assert!(matches!(nav, Nav::Run(Task::Detail(uid)) if uid == "gamma_3x5"));
        assert!(matches!(press(&mut screen, &ctx, KeyCode::Esc), Nav::Pop));
    }
}
