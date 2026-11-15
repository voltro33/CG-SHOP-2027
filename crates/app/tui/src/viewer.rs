//! The browser view of one or more tours: a self-contained HTML page with
//! the `mow-viz` pictures inline, panned and zoomed together, each cutter's
//! path shown or hidden by a checkbox. The page is a local file under
//! `data/rust/view/`; nothing is published.

use std::path::{Path, PathBuf};

use mow_core::{Problem, Solution};
use mow_viz::Scene;

/// One picture on the page.
#[derive(Debug, Clone)]
pub struct Panel {
    pub heading: String,
    pub svg: String,
}

impl Panel {
    /// The solution drawn over its instance, headed by `name` and its
    /// tour lengths.
    pub fn of(name: &str, problem: &Problem, solution: &Solution) -> Self {
        let lengths: Vec<String> = solution.routes().iter().map(|r| r.length().to_string()).collect();
        let heading = format!(
            "{name}: longest {}, tours {}",
            solution.objective(),
            lengths.join(" / ")
        );
        let svg = Scene::new(problem).routes(solution.routes()).to_svg();

        Self { heading, svg }
    }
}

/// The whole page. `cutters` is how many checkboxes to offer.
pub fn page(title: &str, cutters: usize, panels: &[Panel]) -> String {
    let toggles: String = (0..cutters)
        .map(|i| {
            format!(
                "<label><input type=\"checkbox\" checked data-toggle=\"{i}\"> cutter {}</label>",
                i + 1
            )
        })
        .collect();
    let figures: String = panels
        .iter()
        .map(|p| {
            format!(
                "<figure><figcaption>{}</figcaption>{}</figure>",
                escape(&p.heading),
                strip_prolog(&p.svg)
            )
        })
        .collect();

    format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>{title}</title><style>{STYLE}</style></head><body>\
         <header><h1>{title}</h1><div class=\"controls\">{toggles}\
         <button id=\"reset\">reset view</button>\
         <span class=\"hint\">drag to pan, wheel to zoom</span></div></header>\
         <main class=\"panels n{n}\">{figures}</main><script>{SCRIPT}</script></body></html>\n",
        title = escape(title),
        n = panels.len().min(2),
    )
}

/// Writes the page as `<view dir>/<name>.html` and returns its path.
pub fn write(dir: &Path, name: &str, html: &str) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;

    let path = dir.join(format!("{}.html", mow_store::safe_name(name)));
    std::fs::write(&path, html).map_err(|e| format!("cannot write {}: {e}", path.display()))?;

    Ok(path)
}

/// Opens `path` in the default browser, without waiting for it.
pub fn open(path: &Path) -> Result<(), String> {
    let target = path.to_string_lossy().into_owned();
    let mut command = if cfg!(windows) {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", "", &target]);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("open");
        c.arg(&target);
        c
    } else {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(&target);
        c
    };

    command
        .spawn()
        .map(drop)
        .map_err(|e| format!("cannot open {target}: {e}"))
}

/// The SVG without its XML declaration, to sit inside HTML.
fn strip_prolog(svg: &str) -> &str {
    match svg.find("<svg") {
        Some(start) => &svg[start..],
        None => svg,
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const STYLE: &str = "\
:root{color-scheme:light;--ink:#222;--muted:#666;--line:#ddd;--bg:#fafafa}\
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--ink);\
font:14px/1.4 system-ui,sans-serif}header{padding:12px 16px;border-bottom:1px solid var(--line);\
background:#fff;position:sticky;top:0;z-index:1}h1{font-size:16px;margin:0 0 6px}\
.controls{display:flex;flex-wrap:wrap;gap:12px;align-items:center}\
.hint{color:var(--muted)}button{font:inherit;padding:2px 10px}\
.panels{display:grid;gap:12px;padding:12px 16px}.panels.n2{grid-template-columns:1fr 1fr}\
@media (max-width:900px){.panels.n2{grid-template-columns:1fr}}\
figure{margin:0;background:#fff;border:1px solid var(--line);border-radius:6px;overflow:hidden}\
figcaption{padding:6px 10px;border-bottom:1px solid var(--line);color:var(--muted)}\
svg{display:block;width:100%;height:auto;max-height:82vh;cursor:grab;touch-action:none}\
svg.dragging{cursor:grabbing}";

/// Pan and zoom by moving each picture's viewBox by the same fractions, so
/// pictures of different extents stay in step; checkboxes hide a cutter.
const SCRIPT: &str = r#"
const svgs = [...document.querySelectorAll('main svg')];
const start = svgs.map(s => s.viewBox.baseVal && [s.viewBox.baseVal.x, s.viewBox.baseVal.y, s.viewBox.baseVal.width, s.viewBox.baseVal.height]);
const set = (s, v) => s.setAttribute('viewBox', v.join(' '));
const box = s => { const b = s.viewBox.baseVal; return [b.x, b.y, b.width, b.height]; };
function zoomAll(fx, fy, factor) {
  for (const s of svgs) { const [x, y, w, h] = box(s);
    const px = x + fx * w, py = y + fy * h;
    set(s, [px - (px - x) * factor, py - (py - y) * factor, w * factor, h * factor]); }
}
function panAll(dx, dy) {
  for (const s of svgs) { const [x, y, w, h] = box(s); set(s, [x - dx * w, y - dy * h, w, h]); }
}
for (const s of svgs) {
  s.addEventListener('wheel', e => { e.preventDefault(); const r = s.getBoundingClientRect();
    zoomAll((e.clientX - r.left) / r.width, (e.clientY - r.top) / r.height, e.deltaY > 0 ? 1.2 : 1 / 1.2); }, { passive: false });
  let last = null;
  s.addEventListener('pointerdown', e => { last = [e.clientX, e.clientY]; s.setPointerCapture(e.pointerId); s.classList.add('dragging'); });
  s.addEventListener('pointermove', e => { if (!last) return; const r = s.getBoundingClientRect();
    panAll((e.clientX - last[0]) / r.width, (e.clientY - last[1]) / r.height); last = [e.clientX, e.clientY]; });
  const stop = () => { last = null; s.classList.remove('dragging'); };
  s.addEventListener('pointerup', stop); s.addEventListener('pointercancel', stop);
}
document.getElementById('reset').addEventListener('click', () => svgs.forEach((s, i) => start[i] && set(s, start[i])));
for (const box of document.querySelectorAll('[data-toggle]')) {
  box.addEventListener('change', () => document.querySelectorAll(`.route[data-cutter="${box.dataset.toggle}"]`)
    .forEach(p => p.style.display = box.checked ? '' : 'none'));
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_holds_each_picture_without_its_prolog() {
        let panels = [
            Panel {
                heading: "best: longest 10".to_owned(),
                svg: "<?xml version=\"1.0\"?>\n<svg viewBox=\"0 0 1 1\"></svg>".to_owned(),
            },
            Panel {
                heading: "candidate <a>".to_owned(),
                svg: "<svg viewBox=\"0 0 2 2\"></svg>".to_owned(),
            },
        ];

        let html = page("x & y", 2, &panels);

        assert!(html.starts_with("<!doctype html>"));
        assert!(!html.contains("<?xml"));
        assert_eq!(html.matches("<figure>").count(), 2);
        assert!(html.contains("candidate &lt;a&gt;"));
        assert!(html.contains("<title>x &amp; y</title>"));
        assert!(html.contains("data-toggle=\"1\""));
        assert!(html.contains("panels n2"));
    }
}
