//! Raw XML capture and helpers shared by reader/writer.

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, Writer};

use crate::{Error, Result};

/// Qualified name of a start tag as written (e.g. `w:tbl`).
pub fn qname(e: &BytesStart<'_>) -> String {
    e.name().into_inner().to_string()
}

/// Local name without prefix.
pub fn local(e: &BytesStart<'_>) -> String {
    e.local_name().into_inner().to_string()
}

/// Serialise `start` and everything up to its matching end tag verbatim.
/// `empty` = the start event was self-closing.
pub fn capture(reader: &mut Reader<&[u8]>, start: &BytesStart<'_>, empty: bool) -> Result<String> {
    let mut w = Writer::new(Vec::new());
    if empty {
        w.write_event(Event::Empty(start.borrow()))
            .map_err(Error::xml)?;
        return Ok(String::from_utf8_lossy(&w.into_inner()).into_owned());
    }
    w.write_event(Event::Start(start.borrow()))
        .map_err(Error::xml)?;
    let mut depth = 1usize;
    loop {
        let ev = reader.read_event().map_err(Error::xml)?;
        match &ev {
            Event::Start(_) => depth += 1,
            Event::End(_) => depth -= 1,
            Event::Eof => return Err(Error::xml("unexpected EOF while capturing element")),
            _ => {}
        }
        w.write_event(ev).map_err(Error::xml)?;
        if depth == 0 {
            break;
        }
    }
    Ok(String::from_utf8_lossy(&w.into_inner()).into_owned())
}

/// Skip to the end of the current element.
pub fn skip(reader: &mut Reader<&[u8]>, empty: bool) -> Result<()> {
    if empty {
        return Ok(());
    }
    let mut depth = 1usize;
    loop {
        match reader.read_event().map_err(Error::xml)? {
            Event::Start(_) => depth += 1,
            Event::End(_) => {
                depth -= 1;
                if depth == 0 {
                    return Ok(());
                }
            }
            Event::Eof => return Err(Error::xml("unexpected EOF")),
            _ => {}
        }
    }
}

/// `w:val`-style attribute lookup by local name.
pub fn attr(e: &BytesStart<'_>, name: &str) -> Option<String> {
    prism_openxml::xml::attr(&prism_openxml::xml::attrs_of(e), name).map(str::to_owned)
}

/// OOXML on/off property: absent `w:val`, `true`, `1`, `on` = true.
pub fn on_off(e: &BytesStart<'_>) -> bool {
    !matches!(
        attr(e, "val").as_deref(),
        Some("0" | "false" | "off" | "none")
    )
}

pub fn escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
    out
}

pub use prism_openxml::xml::escape_attr;
