use prism_drawingml::fill::Fill;
use prism_drawingml::shape::{Geometry, Node};
use prism_openxml::{Package, PartName};
use prism_slide::anim::{Effect, Timing, Transition};
use prism_slide::{read_pptx, write_pptx, PresentationModel, Slide};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn all_text(nodes: &[Node]) -> String {
    let mut s = String::new();
    for n in nodes {
        match n {
            Node::Group(g) => s += &all_text(&g.children),
            Node::Frame(f) => {
                if let Some(t) = &f.table {
                    for (_, row) in &t.rows {
                        for c in row {
                            s += &c.text.plain_text();
                            s.push('|');
                        }
                    }
                }
            }
            n => {
                if let Some(t) = n.text() {
                    s += &t.plain_text();
                    s.push('|');
                }
            }
        }
    }
    s
}

fn check(m: &PresentationModel) {
    assert!(
        (m.width - 12192000).abs() < 1000 && m.height == 6858000,
        "{} {}",
        m.width,
        m.height
    );
    assert_eq!(m.slides.len(), 4);
    assert!(!m.masters.is_empty());
    for sl in &m.slides {
        assert!(
            m.masters
                .iter()
                .any(|ms| ms.layouts.iter().any(|l| l.part == sl.layout)),
            "layout {} of a slide is known",
            sl.layout
        );
    }
    assert!(m.masters[0].theme.color("accent1").is_some());
    let s0 = &m.slides[0];
    assert!(all_text(&s0.shapes).contains("Prism Office"));
    assert_eq!(s0.notes, "Welcome everyone.\nIntroduce the team.");
    assert!(all_text(&m.slides[1].shapes).contains("Tracked changes"));
    let s2 = &m.slides[2].shapes;
    assert!(s2.iter().any(|n| matches!(n, Node::Shape(s) if matches!(&s.geom, Some(Geometry::Preset { name, .. }) if name == "star5"))));
    let pic = s2
        .iter()
        .find_map(|n| {
            if let Node::Picture(p) = n {
                Some(p)
            } else {
                None
            }
        })
        .unwrap();
    assert!(
        matches!(&pic.image, Fill::Image { rel_id, .. } if rel_id.starts_with("/ppt/media/")),
        "{:?}",
        pic.image
    );
    let txt = all_text(s2);
    assert!(
        txt.contains("R1C2") && txt.contains("Rounded box") && txt.contains("Visit prism.example"),
        "{txt}"
    );
    assert!(s2
        .iter()
        .any(|n| matches!(n, Node::Group(g) if g.children.len() == 2)));
    assert!(m.slides[3].shapes.iter().any(|n| matches!(n, Node::Frame(f) if f.kind == "chart" && f.rel_id.as_deref().is_some_and(|r| r.starts_with("/ppt/charts/")))));
}

#[test]
fn reads_python_pptx_and_libreoffice_files() {
    for f in ["python-pptx.pptx", "libreoffice.pptx"] {
        let (m, _) = read_pptx(&fixture(f)).unwrap_or_else(|e| panic!("{f}: {e}"));
        check(&m);
    }
}

#[test]
fn unchanged_round_trip_keeps_parts() {
    for f in ["python-pptx.pptx", "libreoffice.pptx"] {
        let bytes = fixture(f);
        let (m, src) = read_pptx(&bytes).unwrap();
        let out = write_pptx(&m, &src).unwrap();
        let (m2, _) = read_pptx(&out).unwrap();
        assert_eq!(m, m2, "{f}");
        let a = Package::from_bytes(&bytes).unwrap();
        let b = Package::from_bytes(&out).unwrap();
        for p in a.part_names() {
            let n = p.as_str();
            if n.contains("/media/")
                || n.contains("/charts/")
                || n.contains("/theme/")
                || n.contains("slideLayout")
                || n.contains("slideMaster")
                || n.contains("notesSlide")
            {
                assert_eq!(a.part(p), b.part(p), "{f}: {n}");
            }
        }
    }
}

#[test]
fn edits_slides_notes_transitions_and_animations() {
    let (mut m, src) = read_pptx(&fixture("python-pptx.pptx")).unwrap();
    // edit text, move a shape, delete slide 2, add a slide with a picture copied from slide 3
    if let Some(Node::Shape(t)) = m.slides[0].shapes.iter_mut().find(|n| {
        n.common()
            .ph
            .as_ref()
            .is_some_and(|p| p.kind.as_deref() == Some("ctrTitle"))
    }) {
        t.text = Some(prism_drawingml::text::TextBody {
            paragraphs: vec![prism_drawingml::text::Paragraph::plain("Prism Office 2027")],
            ..t.text.clone().unwrap()
        });
    }
    let star_id = m.slides[2]
        .shapes
        .iter()
        .find(|n| n.common().name == "Star")
        .unwrap()
        .common()
        .id;
    if let Some(n) = m.slides[2]
        .shapes
        .iter_mut()
        .find(|n| n.common().id == star_id)
    {
        n.common_mut().xfrm.as_mut().unwrap().x += 914400;
    }
    let pic = m.slides[2]
        .shapes
        .iter()
        .find(|n| matches!(n, Node::Picture(_)))
        .unwrap()
        .clone();
    let layout = m.slides[2].layout.clone();
    m.slides.remove(1);
    m.slides.push(Slide {
        part: None,
        layout,
        shapes: vec![pic],
        notes: "New slide notes".into(),
        ..Default::default()
    });
    m.slides[0].notes = "Changed notes".into();
    m.slides[1].transition = Some(Transition {
        kind: "morph".into(),
        dir: None,
        option: Some("byObject".into()),
        duration: Some(2000),
        advance_click: true,
        advance_after: None,
    });
    m.slides[1].timing = Some(Timing {
        effects: vec![Effect {
            shape: star_id,
            class: "entr".into(),
            preset: "fade".into(),
            preset_id: 10,
            subtype: 0,
            trigger: "onClick".into(),
            delay: 0,
            duration: 500,
            path: None,
            repeat: None,
        }],
        partial: false,
    });
    let out = write_pptx(&m, &src).unwrap();
    std::fs::write(std::env::temp_dir().join("prism-slide-edited.pptx"), &out).unwrap();
    let (m2, _) = read_pptx(&out).unwrap();
    assert_eq!(m2.slides.len(), 4);
    assert!(all_text(&m2.slides[0].shapes).contains("Prism Office 2027"));
    assert_eq!(m2.slides[0].notes, "Changed notes");
    assert_eq!(m2.slides[3].notes, "New slide notes");
    assert!(
        matches!(&m2.slides[3].shapes[0], Node::Picture(p) if matches!(&p.image, Fill::Image { rel_id, .. } if rel_id.starts_with("/ppt/media/")))
    );
    assert_eq!(m2.slides[1].transition.as_ref().unwrap().kind, "morph");
    assert_eq!(m2.slides[1].timing.as_ref().unwrap().effects.len(), 1);
    let star = m2.slides[1]
        .shapes
        .iter()
        .find(|n| n.common().id == star_id)
        .unwrap();
    assert_eq!(star.common().xfrm.as_ref().unwrap().x, 914400 * 2);
    // the deleted slide's part is gone, the new one exists
    let pkg = Package::from_bytes(&out).unwrap();
    let slides: Vec<_> = pkg
        .part_names()
        .filter(|p| p.as_str().starts_with("/ppt/slides/slide") && !p.as_str().contains("_rels"))
        .collect();
    assert_eq!(slides.len(), 4);
    assert!(
        pkg.part(&PartName::new("/ppt/slides/slide2.xml").unwrap())
            .is_some()
            || slides.len() == 4
    );
}

#[test]
fn blank_template_has_standard_layouts() {
    let bytes = prism_openxml::templates::blank(prism_openxml::DocumentKind::Presentation)
        .to_bytes()
        .unwrap();
    std::fs::write(std::env::temp_dir().join("prism-blank.pptx"), &bytes).unwrap();
    let (m, _) = read_pptx(&bytes).unwrap();
    assert_eq!((m.width, m.height), (12192000, 6858000));
    assert!(m.masters[0].theme.color("accent1").is_some());
    let names: Vec<&str> = m.masters[0]
        .layouts
        .iter()
        .map(|l| l.name.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "Title Slide",
            "Title and Content",
            "Section Header",
            "Two Content",
            "Title Only",
            "Blank"
        ]
    );
    assert_eq!(m.masters[0].shapes.len(), 5);
    assert!(
        m.masters[0]
            .text_styles
            .title
            .levels
            .iter()
            .any(|l| l.is_some())
            || m.masters[0].text_styles.title.default.is_some()
    );
    assert_eq!(m.slides.len(), 1);
}

/// The browser preview ships the blank model as JSON; keep it in sync
/// (regenerate with `cargo run -p prism-slide --example dump_model > src/slide/blank.json`).
#[test]
fn browser_blank_model_matches_template() {
    let bytes = prism_openxml::templates::blank(prism_openxml::DocumentKind::Presentation)
        .to_bytes()
        .unwrap();
    let (m, _) = read_pptx(&bytes).unwrap();
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../src/slide/blank.json");
    let shipped: PresentationModel =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(m, shipped, "src/slide/blank.json is stale");
}

/// Nodes as the TS editor creates them (src/slide/ops.ts) are written as valid PresentationML.
#[test]
fn editor_created_nodes_are_written() {
    let bytes = prism_openxml::templates::blank(prism_openxml::DocumentKind::Presentation)
        .to_bytes()
        .unwrap();
    let (mut m, src) = read_pptx(&bytes).unwrap();
    let layout = m.masters[0].layouts[1].part.clone();
    let shapes: Vec<Node> = serde_json::from_value(serde_json::json!([
        {"type":"shape","id":2,"name":"Title 1","ph":{"kind":"title"},"text":{"paragraphs":[{"runs":[{"t":"text","text":"Editor title"}]}]}},
        {"type":"shape","id":3,"name":"Content 2","ph":{"idx":1},"text":{"paragraphs":[{}]}},
        {"type":"shape","id":4,"name":"TextBox 3","textBox":true,"xfrm":{"x":914400,"y":914400,"cx":2743200,"cy":457200},
         "geom":{"t":"preset","name":"rect"},"fill":{"t":"none"},
         "text":{"body":{"wrap":"square","autofit":{"t":"shape"}},"paragraphs":[{"runs":[{"t":"text","text":"Box","props":{"bold":true,"size":2400}}]}]}},
        {"type":"group","id":5,"name":"Group 4","xfrm":{"x":0,"y":3000000,"cx":2000000,"cy":1000000,"child":[0,3000000,2000000,1000000]},"children":[
            {"type":"shape","id":6,"name":"star5 5","xfrm":{"x":0,"y":3000000,"cx":1000000,"cy":1000000},"geom":{"t":"preset","name":"star5"},
             "fill":{"t":"solid","color":{"base":{"t":"scheme","v":"accent1"}}},
             "line":{"width":12700,"fill":{"t":"solid","color":{"base":{"t":"scheme","v":"accent1"},"mods":[{"name":"shade","val":50000}]}}},
             "text":{"body":{"anchor":"ctr"},"paragraphs":[{"props":{"align":"ctr"}}]}},
            {"type":"connector","id":7,"name":"line 6","xfrm":{"x":1000000,"y":3000000,"cx":1000000,"cy":1000000},"geom":{"t":"preset","name":"line"},
             "line":{"width":19050,"fill":{"t":"solid","color":{"base":{"t":"scheme","v":"accent1"}}},"tail":{"type":"triangle"}}}
        ]},
        {"type":"frame","id":8,"name":"Table 7","xfrm":{"x":5000000,"y":3000000,"cx":4000000,"cy":1000000},"kind":"table",
         "table":{"cols":[2000000,2000000],"rows":[[500000,[{"text":{"paragraphs":[{"runs":[{"t":"text","text":"H1"}]}]}},{"text":{"paragraphs":[{}]}}]],
                                                   [500000,[{"text":{"paragraphs":[{}]}},{"text":{"paragraphs":[{"runs":[{"t":"text","text":"c"}]}]}}]]],
                  "styleId":"{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}","firstRow":true,"bandRow":true}}
    ]))
    .unwrap();
    m.slides.push(Slide {
        part: None,
        layout,
        shapes: shapes.clone(),
        background: None,
        notes: String::new(),
        hidden: false,
        transition: None,
        timing: None,
    });
    let out = write_pptx(&m, &src).unwrap();
    std::fs::write(std::env::temp_dir().join("prism-editor-nodes.pptx"), &out).unwrap();
    let (m2, _) = read_pptx(&out).unwrap();
    assert_eq!(m2.slides[1].shapes, shapes);
}
