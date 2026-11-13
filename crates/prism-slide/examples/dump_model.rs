//! Print the JSON model of a .pptx (or of the blank template with no argument).
fn main() {
    let bytes = match std::env::args().nth(1) {
        Some(p) => std::fs::read(p).expect("read"),
        None => prism_openxml::templates::blank(prism_openxml::DocumentKind::Presentation)
            .to_bytes()
            .unwrap(),
    };
    let (m, _) = prism_slide::read_pptx(&bytes).expect("pptx");
    println!("{}", serde_json::to_string_pretty(&m).unwrap());
}
