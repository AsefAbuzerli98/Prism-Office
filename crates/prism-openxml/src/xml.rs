//! Small XML helpers on top of quick-xml.

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use crate::{Error, Result};

pub(crate) const XML_DECL: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;

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

pub(crate) fn attr<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

/// Escape a string for use inside a double-quoted XML attribute.
pub(crate) fn escape_attr(s: &str) -> String {
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
