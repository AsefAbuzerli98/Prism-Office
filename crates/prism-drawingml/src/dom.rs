//! A tiny XML element tree that keeps qualified names, attribute order and
//! unknown content, so DrawingML fragments round-trip unchanged.

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

/// Maximum element nesting accepted by [`El::parse`].
pub const MAX_DEPTH: usize = 256;

#[derive(Clone, Debug, PartialEq)]
pub enum XNode {
    El(El),
    Text(String),
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct El {
    /// Qualified name as written (`a:solidFill`).
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<XNode>,
}

fn esc_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn esc_attr(s: &str) -> String {
    esc_text(s).replace('"', "&quot;")
}

fn unesc(s: &str) -> String {
    quick_xml::escape::unescape(s)
        .map(|c| c.into_owned())
        .unwrap_or_else(|_| s.to_string())
}

fn start(e: &BytesStart<'_>) -> El {
    let name = e.name().as_ref().to_string();
    let attrs = e
        .attributes()
        .filter_map(|a| a.ok())
        .map(|a| (a.key.as_ref().to_string(), unesc(a.value.as_ref())))
        .collect();
    El {
        name,
        attrs,
        children: vec![],
    }
}

impl El {
    pub fn new(name: &str) -> Self {
        El {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Parse the first element of an XML fragment or document. Documents
    /// nested deeper than [`MAX_DEPTH`] are rejected (the tree is walked
    /// recursively elsewhere; real OOXML stays far below this).
    pub fn parse(xml: &str) -> Option<El> {
        let mut r = Reader::from_str(xml);
        let mut stack: Vec<El> = Vec::new();
        loop {
            match r.read_event().ok()? {
                Event::Start(e) => {
                    if stack.len() >= MAX_DEPTH {
                        return None;
                    }
                    stack.push(start(&e))
                }
                Event::Empty(e) => {
                    let el = start(&e);
                    match stack.last_mut() {
                        Some(p) => p.children.push(XNode::El(el)),
                        None => return Some(el),
                    }
                }
                Event::End(_) => {
                    let el = stack.pop()?;
                    match stack.last_mut() {
                        Some(p) => p.children.push(XNode::El(el)),
                        None => return Some(el),
                    }
                }
                Event::Text(t) => {
                    if let Some(p) = stack.last_mut() {
                        let s = t.into_inner().to_string();
                        push_text(p, &s);
                    }
                }
                Event::GeneralRef(g) => {
                    if let Some(p) = stack.last_mut() {
                        let raw = g.into_inner().to_string();
                        push_text(p, &unesc(&format!("&{raw};")));
                    }
                }
                Event::CData(c) => {
                    if let Some(p) = stack.last_mut() {
                        push_text(p, &c.into_inner());
                    }
                }
                Event::Eof => return None,
                _ => {}
            }
        }
    }

    /// Local name without prefix.
    pub fn local(&self) -> &str {
        self.name.rsplit(':').next().unwrap_or(&self.name)
    }

    pub fn prefix(&self) -> Option<&str> {
        self.name.split_once(':').map(|(p, _)| p)
    }

    pub fn attr(&self, k: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(n, _)| n == k || n.rsplit(':').next() == Some(k) && !k.contains(':'))
            .map(|(_, v)| v.as_str())
    }

    pub fn attr_i64(&self, k: &str) -> Option<i64> {
        self.attr(k)
            .and_then(|v| v.parse::<f64>().ok())
            .map(|v| v as i64)
    }

    pub fn attr_f64(&self, k: &str) -> Option<f64> {
        self.attr(k).and_then(|v| v.parse().ok())
    }

    pub fn attr_bool(&self, k: &str) -> Option<bool> {
        self.attr(k).map(|v| v == "1" || v == "true" || v == "on")
    }

    pub fn set(&mut self, k: &str, v: impl ToString) -> &mut Self {
        let v = v.to_string();
        match self.attrs.iter_mut().find(|(n, _)| n == k) {
            Some(a) => a.1 = v,
            None => self.attrs.push((k.into(), v)),
        }
        self
    }

    pub fn with(mut self, k: &str, v: impl ToString) -> Self {
        self.set(k, v);
        self
    }

    pub fn child(mut self, c: El) -> Self {
        self.children.push(XNode::El(c));
        self
    }

    pub fn push(&mut self, c: El) {
        self.children.push(XNode::El(c));
    }

    pub fn text_child(mut self, t: &str) -> Self {
        self.children.push(XNode::Text(t.into()));
        self
    }

    pub fn elements(&self) -> impl Iterator<Item = &El> {
        self.children
            .iter()
            .filter_map(|c| if let XNode::El(e) = c { Some(e) } else { None })
    }

    /// First child element with this local name.
    pub fn find(&self, local: &str) -> Option<&El> {
        self.elements().find(|e| e.local() == local)
    }

    pub fn find_all<'a>(&'a self, local: &'a str) -> impl Iterator<Item = &'a El> {
        self.elements().filter(move |e| e.local() == local)
    }

    /// Descendant by a path of local names.
    pub fn path(&self, path: &[&str]) -> Option<&El> {
        let mut cur = self;
        for p in path {
            cur = cur.find(p)?;
        }
        Some(cur)
    }

    pub fn text(&self) -> String {
        let mut s = String::new();
        for c in &self.children {
            match c {
                XNode::Text(t) => s.push_str(t),
                XNode::El(e) => s.push_str(&e.text()),
            }
        }
        s
    }

    pub fn to_xml(&self) -> String {
        let mut s = String::new();
        self.write(&mut s);
        s
    }

    fn write(&self, s: &mut String) {
        s.push('<');
        s.push_str(&self.name);
        for (k, v) in &self.attrs {
            s.push(' ');
            s.push_str(k);
            s.push_str("=\"");
            s.push_str(&esc_attr(v));
            s.push('"');
        }
        if self.children.is_empty() {
            s.push_str("/>");
            return;
        }
        s.push('>');
        for c in &self.children {
            match c {
                XNode::Text(t) => s.push_str(&esc_text(t)),
                XNode::El(e) => e.write(s),
            }
        }
        s.push_str("</");
        s.push_str(&self.name);
        s.push('>');
    }
}

fn push_text(p: &mut El, s: &str) {
    if let Some(XNode::Text(t)) = p.children.last_mut() {
        t.push_str(s);
    } else {
        p.children.push(XNode::Text(s.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_fragments() {
        let x = r#"<a:p xmlns:a="urn:a"><a:r><a:rPr lang="az-AZ" b="1"/><a:t>Salam &amp; &lt;dünya&gt;</a:t></a:r><a:endParaRPr/></a:p>"#;
        let e = El::parse(x).unwrap();
        assert_eq!(e.local(), "p");
        assert_eq!(e.path(&["r", "t"]).unwrap().text(), "Salam & <dünya>");
        assert_eq!(e.path(&["r", "rPr"]).unwrap().attr("lang"), Some("az-AZ"));
        assert_eq!(e.to_xml(), x);
    }
}
