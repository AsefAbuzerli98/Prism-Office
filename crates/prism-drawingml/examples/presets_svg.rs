//! Writes an HTML sheet of every preset (visual checks against other renderers).
use prism_drawingml::geom::{evaluate_preset, preset_names, Cmd, PathFill};

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "presets.html".into());
    let mut html = String::from("<html><body style='margin:0;font:11px sans-serif'><div style='display:flex;flex-wrap:wrap'>");
    for name in preset_names() {
        let e = evaluate_preset(name, 120.0, 80.0, &[]);
        let mut svg = String::new();
        for p in &e.paths {
            let mut d = String::new();
            for c in &p.cmds {
                match c {
                    Cmd::M { x, y } => d += &format!("M{x:.2} {y:.2}"),
                    Cmd::L { x, y } => d += &format!("L{x:.2} {y:.2}"),
                    Cmd::C {
                        x1,
                        y1,
                        x2,
                        y2,
                        x,
                        y,
                    } => d += &format!("C{x1:.2} {y1:.2} {x2:.2} {y2:.2} {x:.2} {y:.2}"),
                    Cmd::Q { x1, y1, x, y } => d += &format!("Q{x1:.2} {y1:.2} {x:.2} {y:.2}"),
                    Cmd::Z => d += "Z",
                }
            }
            let fill = match p.fill {
                PathFill::None => "none",
                PathFill::Darken | PathFill::DarkenLess => "#2f5597",
                PathFill::Lighten | PathFill::LightenLess => "#8faadc",
                PathFill::Norm => "#4472c4",
            };
            let stroke = if p.stroke { "#203864" } else { "none" };
            svg += &format!("<path d='{d}' fill='{fill}' stroke='{stroke}' stroke-width='1.5'/>");
        }
        html += &format!("<div style='width:150px;height:120px;text-align:center'><svg width='140' height='96' viewBox='-10 -8 140 96'>{svg}</svg><br>{name}</div>");
    }
    html += "</div></body></html>";
    std::fs::write(out, html).unwrap();
}
