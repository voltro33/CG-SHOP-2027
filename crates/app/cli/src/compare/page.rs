//! The summary page of a folder comparison.
//!
//! A header with the totals, then a section per cutter family and per k
//! (each a card; clicking one filters the table), then one table of every
//! instance: compared ones linked to their side by side page, the ones left
//! out with the reason. The table sorts by any column and filters by result
//! and by name. One self-contained file, no outside scripts.

use mow_report::family::family;

use super::folders::{FolderComparison, Group};
use super::pair::Pair;

/// What one row of the table is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    BShorter,
    AShorter,
    Tied,
    Invalid,
    OnlyA,
    OnlyB,
}

impl Status {
    fn of(pair: &Pair) -> Self {
        match pair.b.objective().cmp(&pair.a.objective()) {
            std::cmp::Ordering::Less => Self::BShorter,
            std::cmp::Ordering::Greater => Self::AShorter,
            std::cmp::Ordering::Equal => Self::Tied,
        }
    }

    /// The `data-status` value the filters match.
    fn key(self) -> &'static str {
        match self {
            Self::BShorter => "b",
            Self::AShorter => "a",
            Self::Tied => "tied",
            Self::Invalid | Self::OnlyA | Self::OnlyB => "out",
        }
    }
}

/// The whole page. `pages` is the folder, next to the page, that holds one
/// side by side page per compared instance.
pub fn summary_page(comparison: &FolderComparison, paths: (&str, &str), pages: &str) -> String {
    let (a, b) = (&comparison.label_a, &comparison.label_b);
    let groups = comparison.groups();
    let all = &groups[0];

    let families: Vec<&Group> = groups.iter().filter(|g| is_family(g)).collect();
    let ks: Vec<&Group> = groups.iter().filter(|g| g.name.starts_with("k = ")).collect();
    let left_out = comparison.invalid.len() + comparison.only_a.len() + comparison.only_b.len();

    let title = format!("{a} against {b}");
    let header = format!(
        "<header><h1>{title}</h1>\
         <p class=\"paths\"><span class=\"tag ta\">A</span> {pa}<br><span class=\"tag tb\">B</span> {pb}</p>\
         {kpis}</header>",
        title = escape(&title),
        pa = escape(paths.0),
        pb = escape(paths.1),
        kpis = kpis(all, left_out, (a, b)),
    );

    let sections = format!(
        "<section><h2>By cutter family</h2><div class=\"cards\">{}</div></section>\
         <section><h2>By number of cutters</h2><div class=\"cards\">{}</div></section>",
        families
            .iter()
            .map(|g| card(g, "family", &g.name))
            .collect::<String>(),
        ks.iter()
            .map(|g| card(g, "k", g.name.trim_start_matches("k = ")))
            .collect::<String>(),
    );

    let toolbar = format!(
        "<div class=\"toolbar\"><input id=\"q\" type=\"search\" placeholder=\"find an instance\">\
         <div class=\"chips\">\
         <button class=\"chip on\" data-status=\"all\">all {n}</button>\
         <button class=\"chip\" data-status=\"b\">{b} shorter {bs}</button>\
         <button class=\"chip\" data-status=\"a\">{a} shorter {as_}</button>\
         <button class=\"chip\" data-status=\"tied\">tied {t}</button>\
         <button class=\"chip\" data-status=\"out\">left out {out}</button></div>\
         <span id=\"scope\"></span><span id=\"count\"></span></div>",
        n = all.instances + left_out,
        a = escape(a),
        b = escape(b),
        bs = all.b_shorter,
        as_ = all.a_shorter,
        t = all.tied,
        out = left_out,
    );

    format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>{title}</title><style>{STYLE}</style></head><body>{header}<main>{sections}\
         <section><h2>Instances</h2>{toolbar}<div class=\"scroll\">{table}</div></section>\
         </main><script>{SCRIPT}</script></body></html>\n",
        title = escape(&title),
        table = table(comparison, pages),
    )
}

fn is_family(group: &Group) -> bool {
    group.name != "all" && !group.name.starts_with("k = ")
}

/// The totals across the top.
fn kpis(all: &Group, left_out: usize, (a, b): (&str, &str)) -> String {
    let tile = |label: &str, value: String, class: &str| {
        format!(
            "<div class=\"kpi {class}\"><div class=\"label\">{}</div><div class=\"value\">{value}</div></div>",
            escape(label)
        )
    };

    let tiles = [
        tile("instances compared", all.instances.to_string(), ""),
        tile(&format!("points A: {a}"), format!("{:.3}", all.points_a), ""),
        tile(&format!("points B: {b}"), format!("{:.3}", all.points_b), ""),
        tile("B - A", format!("{:+.4}", all.gain()), sign_class(all.gain())),
        tile(&format!("{b} shorter"), all.b_shorter.to_string(), "good"),
        tile(&format!("{a} shorter"), all.a_shorter.to_string(), "bad"),
        tile("tied", all.tied.to_string(), ""),
        tile(
            "left out",
            left_out.to_string(),
            if left_out > 0 { "warn" } else { "" },
        ),
    ]
    .concat();

    format!("<div class=\"kpis\">{tiles}</div>")
}

/// One group as a card: its points, and a bar of who is shorter.
fn card(group: &Group, key: &str, value: &str) -> String {
    let n = group.instances.max(1) as f64;
    let width = |count: usize| format!("{:.1}%", 100.0 * count as f64 / n);

    format!(
        "<button class=\"card\" data-key=\"{key}\" data-value=\"{value}\">\
         <div class=\"name\">{name}</div>\
         <div class=\"gain {class}\">{gain:+.4}</div>\
         <div class=\"sub\">{n} instances, {pa:.2} to {pb:.2} points</div>\
         <div class=\"bar\"><span class=\"sb\" style=\"width:{wb}\"></span>\
         <span class=\"st\" style=\"width:{wt}\"></span><span class=\"sa\" style=\"width:{wa}\"></span></div>\
         <div class=\"legend\"><span>B {b}</span><span>tied {t}</span><span>A {a}</span></div></button>",
        key = key,
        value = escape(value),
        name = escape(&group.name),
        class = sign_class(group.gain()),
        gain = group.gain(),
        n = group.instances,
        pa = group.points_a,
        pb = group.points_b,
        wb = width(group.b_shorter),
        wt = width(group.tied),
        wa = width(group.a_shorter),
        b = group.b_shorter,
        t = group.tied,
        a = group.a_shorter,
    )
}

/// Every instance, compared or left out, in one table.
fn table(comparison: &FolderComparison, pages: &str) -> String {
    let head = [
        ("instance", "text"),
        ("family", "text"),
        ("k", "num"),
        ("A longest", "num"),
        ("B longest", "num"),
        ("B - A", "num"),
        ("change", "num"),
        ("points A", "num"),
        ("points B", "num"),
        ("points B - A", "num"),
        ("result", "text"),
    ]
    .iter()
    .enumerate()
    .map(|(i, (name, kind))| format!("<th data-col=\"{i}\" data-kind=\"{kind}\">{name}</th>"))
    .collect::<String>();

    let mut rows: String = comparison
        .pairs
        .iter()
        .map(|p| pair_row(p, comparison, pages))
        .collect();

    let out = comparison
        .invalid
        .iter()
        .map(|s| (s.name.as_str(), Status::Invalid, s.reason.as_str()))
        .chain(comparison.only_a.iter().map(|n| (n.as_str(), Status::OnlyA, "")))
        .chain(comparison.only_b.iter().map(|n| (n.as_str(), Status::OnlyB, "")));
    for (name, status, reason) in out {
        rows.push_str(&left_out_row(name, status, reason, comparison));
    }

    format!("<table id=\"results\"><thead><tr>{head}</tr></thead><tbody>{rows}</tbody></table>")
}

fn pair_row(pair: &Pair, comparison: &FolderComparison, pages: &str) -> String {
    let (a, b) = (pair.a.objective(), pair.b.objective());
    let diff = i128::from(b) - i128::from(a);
    let change = 100.0 * diff as f64 / a.max(1) as f64;
    let status = Status::of(pair);
    let (pa, pb, gain) = (pair.points(&pair.a), pair.points(&pair.b), pair.gain());

    let result = match status {
        Status::BShorter => format!("{} shorter", comparison.label_b),
        Status::AShorter => format!("{} shorter", comparison.label_a),
        _ => "tied".to_owned(),
    };
    let href = format!("{pages}/{}.html", mow_store::safe_name(pair.uid()));

    format!(
        "<tr data-uid=\"{uid}\" data-family=\"{fam}\" data-k=\"{k}\" data-status=\"{st}\">\
         <td data-v=\"{uid}\"><a href=\"{href}\">{uid}</a></td><td data-v=\"{fam}\">{fam}</td>\
         <td data-v=\"{k}\">{k}</td><td data-v=\"{a}\">{a}</td><td data-v=\"{b}\">{b}</td>\
         <td data-v=\"{diff}\" class=\"{dc}\">{diff:+}</td><td data-v=\"{change}\" class=\"{dc}\">{change:+.2}%</td>\
         <td data-v=\"{pa}\">{pa:.4}</td><td data-v=\"{pb}\">{pb:.4}</td>\
         <td data-v=\"{gain}\" class=\"{gc}\">{gain:+.4}</td>\
         <td data-v=\"{st}\"><span class=\"pill {st}\">{result}</span></td></tr>",
        uid = escape(pair.uid()),
        fam = escape(family(pair.uid())),
        k = pair.problem.k(),
        st = status.key(),
        href = escape(&href),
        dc = sign_class(-change),
        gc = sign_class(gain),
        result = escape(&result),
    )
}

fn left_out_row(name: &str, status: Status, reason: &str, comparison: &FolderComparison) -> String {
    let uid = name.strip_suffix(".solution.json").unwrap_or(name);
    let result = match status {
        Status::Invalid => format!("invalid: {reason}"),
        Status::OnlyA => format!("only in A: {}", comparison.label_a),
        _ => format!("only in B: {}", comparison.label_b),
    };
    let fam = family(uid);

    format!(
        "<tr data-uid=\"{uid}\" data-family=\"{fam}\" data-k=\"\" data-status=\"out\" class=\"out\">\
         <td data-v=\"{uid}\">{uid}</td><td data-v=\"{fam}\">{fam}</td>{empty}\
         <td data-v=\"out\"><span class=\"pill out\">{result}</span></td></tr>",
        uid = escape(uid),
        fam = escape(fam),
        empty = "<td data-v=\"\"></td>".repeat(8),
        result = escape(&result),
    )
}

/// `good` when B gains, `bad` when it loses.
fn sign_class(gain: f64) -> &'static str {
    if gain > 0.0 {
        "good"
    } else if gain < 0.0 {
        "bad"
    } else {
        ""
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const STYLE: &str = "\
:root{color-scheme:light;--bg:#f6f7f9;--panel:#fff;--ink:#1d2329;--muted:#66707a;--line:#e3e6ea;\
--good:#1a7f45;--bad:#c2410c;--warn:#a16207;--tied:#b9c0c7;--accent:#2f5bd3}\
@media (prefers-color-scheme:dark){:root{color-scheme:dark;--bg:#14171a;--panel:#1d2125;--ink:#e6e9ec;\
--muted:#99a2ab;--line:#2e3439;--good:#4cc38a;--bad:#f0884f;--warn:#e0b04a;--tied:#4a5259;--accent:#7f9cf5}}\
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--ink);\
font:14px/1.45 system-ui,-apple-system,Segoe UI,sans-serif}\
header{background:var(--panel);border-bottom:1px solid var(--line);padding:18px 24px 20px}\
h1{font-size:20px;margin:0 0 6px}h2{font-size:13px;text-transform:uppercase;letter-spacing:.06em;\
color:var(--muted);margin:0 0 10px;font-weight:600}\
.paths{margin:0 0 16px;color:var(--muted);font-size:12px;word-break:break-all}\
.tag{display:inline-block;width:18px;text-align:center;border-radius:4px;font-weight:700;color:#fff;font-size:11px}\
.ta{background:var(--bad)}.tb{background:var(--good)}\
.kpis{display:grid;grid-template-columns:repeat(auto-fill,minmax(140px,1fr));gap:10px}\
.kpi{background:var(--bg);border:1px solid var(--line);border-radius:8px;padding:10px 12px}\
.kpi .label{color:var(--muted);font-size:12px}.kpi .value{font-size:22px;font-weight:650;\
font-variant-numeric:tabular-nums}.kpi.good .value,.good{color:var(--good)}.kpi.bad .value,.bad{color:var(--bad)}\
.kpi.warn .value{color:var(--warn)}\
main{padding:20px 24px 40px;display:grid;gap:24px}\
.cards{display:grid;grid-template-columns:repeat(auto-fill,minmax(190px,1fr));gap:10px}\
.card{all:unset;cursor:pointer;background:var(--panel);border:1px solid var(--line);border-radius:8px;\
padding:12px;display:grid;gap:4px}.card:hover{border-color:var(--accent)}\
.card.on{border-color:var(--accent);box-shadow:0 0 0 2px var(--accent) inset}\
.card .name{font-weight:600}.card .gain{font-size:20px;font-weight:650;font-variant-numeric:tabular-nums}\
.card .sub,.legend{color:var(--muted);font-size:12px}\
.bar{display:flex;height:8px;border-radius:4px;overflow:hidden;background:var(--line);margin-top:4px}\
.sb{background:var(--good)}.st{background:var(--tied)}.sa{background:var(--bad)}\
.legend{display:flex;justify-content:space-between}\
.toolbar{display:flex;flex-wrap:wrap;gap:10px;align-items:center;margin-bottom:10px}\
input[type=search]{font:inherit;padding:6px 10px;border:1px solid var(--line);border-radius:6px;\
background:var(--panel);color:var(--ink);min-width:220px}\
.chips{display:flex;flex-wrap:wrap;gap:6px}.chip{font:inherit;font-size:12px;padding:4px 10px;\
border-radius:999px;border:1px solid var(--line);background:var(--panel);color:var(--ink);cursor:pointer}\
.chip.on{background:var(--accent);border-color:var(--accent);color:#fff}\
#scope{font-size:12px;color:var(--accent)}#count{margin-left:auto;color:var(--muted);font-size:12px}\
.scroll{overflow-x:auto;background:var(--panel);border:1px solid var(--line);border-radius:8px}\
table{border-collapse:collapse;width:100%;font-variant-numeric:tabular-nums}\
th,td{padding:6px 10px;border-bottom:1px solid var(--line);text-align:right;white-space:nowrap}\
th:nth-child(-n+2),td:nth-child(-n+2),th:last-child,td:last-child{text-align:left}\
th{position:sticky;top:0;background:var(--panel);color:var(--muted);font-weight:500;cursor:pointer;\
user-select:none}th.asc::after{content:' \\2191'}th.desc::after{content:' \\2193'}\
tbody tr:hover{background:var(--bg)}a{color:var(--accent);text-decoration:none}a:hover{text-decoration:underline}\
tr.out td{color:var(--muted)}\
.pill{font-size:12px;padding:2px 8px;border-radius:999px;border:1px solid var(--line)}\
.pill.b{color:var(--good);border-color:var(--good)}.pill.a{color:var(--bad);border-color:var(--bad)}\
.pill.out{color:var(--warn);border-color:var(--warn);white-space:normal}\
@media (max-width:640px){header,main{padding-left:16px;padding-right:16px}}";

/// Filters (a card, a result chip, the search box) and sorting by column.
const SCRIPT: &str = r#"
const body = document.querySelector('#results tbody');
const rows = [...body.rows];
const state = { key: null, value: null, status: 'all', query: '' };
const count = document.getElementById('count'), scope = document.getElementById('scope');
function apply() {
  let shown = 0;
  for (const r of rows) {
    const ok = (!state.key || r.dataset[state.key] === state.value)
      && (state.status === 'all' || r.dataset.status === state.status)
      && (!state.query || r.dataset.uid.toLowerCase().includes(state.query));
    r.hidden = !ok; if (ok) shown++;
  }
  count.textContent = `${shown} of ${rows.length} shown`;
  scope.textContent = state.key ? `only ${state.key === 'k' ? 'k = ' : ''}${state.value} (click the card again for all)` : '';
}
for (const card of document.querySelectorAll('.card')) card.addEventListener('click', () => {
  const same = state.key === card.dataset.key && state.value === card.dataset.value;
  document.querySelectorAll('.card').forEach(c => c.classList.remove('on'));
  if (same) { state.key = state.value = null; } else { state.key = card.dataset.key; state.value = card.dataset.value; card.classList.add('on'); }
  apply();
});
for (const chip of document.querySelectorAll('.chip')) chip.addEventListener('click', () => {
  document.querySelectorAll('.chip').forEach(c => c.classList.remove('on'));
  chip.classList.add('on'); state.status = chip.dataset.status; apply();
});
document.getElementById('q').addEventListener('input', e => { state.query = e.target.value.trim().toLowerCase(); apply(); });
let sorted = { col: 9, dir: -1 };
for (const th of document.querySelectorAll('#results th')) th.addEventListener('click', () => {
  const col = +th.dataset.col, num = th.dataset.kind === 'num';
  sorted = { col, dir: sorted.col === col ? -sorted.dir : (num ? -1 : 1) };
  const value = r => { const v = r.cells[col].dataset.v; return num ? (v === '' ? -Infinity : +v) : v; };
  rows.sort((x, y) => { const a = value(x), b = value(y); return (a < b ? -1 : a > b ? 1 : 0) * sorted.dir; });
  rows.forEach(r => body.appendChild(r));
  document.querySelectorAll('#results th').forEach(h => h.classList.remove('asc', 'desc'));
  th.classList.add(sorted.dir > 0 ? 'asc' : 'desc');
});
document.querySelector('#results th[data-col="9"]').classList.add('desc');
apply();
"#;
