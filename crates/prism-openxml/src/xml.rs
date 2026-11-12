//! Small XML helpers on top of quick-xml.

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use crate::{Error, Result};

pub const XML_DECL: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;

/// Visit every start/empty element, giving its local name and attributes.
pub(crate) fn for_each_element(
    part: &str,
    data: &[u8],
    mut f: impl FnMut(&str, &[(String, String)]) -> Result<()>,
) -> Result<()> {
    let mut reader = Reader::from_reader(data);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = local_name(&e);
                let attrs = attributes(part, &e)?;
                f(&name, &attrs)?;
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(Error::xml(part, e)),
        }
        buf.clear();
    }
    Ok(())
}

fn local_name(e: &BytesStart<'_>) -> String {
    e.local_name().into_inner().to_owned()
}

fn attributes(part: &str, e: &BytesStart<'_>) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    for attr in e.attributes() {
        let attr = attr.map_err(|err| Error::xml(part, err))?;
        let key = attr.key.local_name().into_inner().to_owned();
        let raw: &str = attr.value.as_ref();
        let value = quick_xml::escape::unescape(raw)
            .map_err(|err| Error::xml(part, err))?
            .into_owned();
        out.push((key, value));
    }
    Ok(out)
}

pub fn attr<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

/// Escape a string for use inside a double-quoted XML attribute.
pub fn escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

/// Text of a `Text` event. With quick-xml ≥0.38 entity references are
/// delivered separately as `GeneralRef` events, so the raw text contains no
/// escapes.
pub fn text_of(e: &quick_xml::events::BytesText<'_>) -> String {
    e.clone().into_inner().into_owned()
}

/// Resolve a `GeneralRef` event (`&amp;`, `&#x41;` …) to its text.
pub fn ref_text(r: &quick_xml::events::BytesRef<'_>) -> String {
    if r.is_char_ref() {
        return r
            .resolve_char_ref()
            .ok()
            .flatten()
            .map(String::from)
            .unwrap_or_default();
    }
    match r.clone().into_inner().as_ref() {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        _ => "",
    }
    .to_string()
}

/// Attributes of an element as (local-name, unescaped value) pairs.
pub fn attrs_of(e: &BytesStart<'_>) -> Vec<(String, String)> {
    attributes("", e).unwrap_or_default()
}

/// Local name of an element.
pub fn name_of(e: &BytesStart<'_>) -> String {
    local_name(e)
}
