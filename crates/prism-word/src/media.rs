//! Pictures and embedded fonts.

use prism_openxml::{rel_types, Package, PartName, TargetMode};
use quick_xml::events::Event;
use quick_xml::Reader;

use crate::raw::{attr, local};
use crate::{Error, Result};

const EMU_PER_PX: u64 = 9525; // 914400 EMU/inch ÷ 96 px/inch
const MAX_CX: u64 = 5_486_400; // 6 inches

/// Pixel size and canonical extension from image bytes (PNG, JPEG, GIF, BMP, WebP).
pub fn image_info(b: &[u8]) -> Option<(u32, u32, &'static str, &'static str)> {
    if b.starts_with(b"\x89PNG\r\n\x1a\n") && b.len() >= 24 {
        let w = u32::from_be_bytes(b[16..20].try_into().ok()?);
        let h = u32::from_be_bytes(b[20..24].try_into().ok()?);
        return Some((w, h, "png", "image/png"));
    }
    if b.starts_with(b"GIF8") && b.len() >= 10 {
        return Some((
            u16::from_le_bytes([b[6], b[7]]) as u32,
            u16::from_le_bytes([b[8], b[9]]) as u32,
            "gif",
            "image/gif",
        ));
    }
    if b.starts_with(b"BM") && b.len() >= 26 {
        let w = i32::from_le_bytes(b[18..22].try_into().ok()?).unsigned_abs();
        let h = i32::from_le_bytes(b[22..26].try_into().ok()?).unsigned_abs();
        return Some((w, h, "bmp", "image/bmp"));
    }
    if b.len() >= 30 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        if &b[12..16] == b"VP8X" {
            let w = 1 + u32::from_le_bytes([b[24], b[25], b[26], 0]);
            let h = 1 + u32::from_le_bytes([b[27], b[28], b[29], 0]);
            return Some((w, h, "webp", "image/webp"));
        }
        return Some((0, 0, "webp", "image/webp"));
    }
    if b.starts_with(&[0xFF, 0xD8]) {
        let mut i = 2;
        while i + 9 < b.len() {
            if b[i] != 0xFF {
                i += 1;
                continue;
            }
            let marker = b[i + 1];
            let len = u16::from_be_bytes([b[i + 2], b[i + 3]]) as usize;
            if (0xC0..=0xCF).contains(&marker) && ![0xC4, 0xC8, 0xCC].contains(&marker) {
                let h = u16::from_be_bytes([b[i + 5], b[i + 6]]) as u32;
                let w = u16::from_be_bytes([b[i + 7], b[i + 8]]) as u32;
                return Some((w, h, "jpeg", "image/jpeg"));
            }
            i += 2 + len;
        }
        return Some((0, 0, "jpeg", "image/jpeg"));
    }
    None
}

fn mime_for(name: &PartName) -> &'static str {
    match name.extension().as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("bmp") => "image/bmp",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("emf") => "image/x-emf",
        Some("wmf") => "image/x-wmf",
        Some("tif" | "tiff") => "image/tiff",
        _ => "application/octet-stream",
    }
}

/// Bytes of a picture referenced by `rel_id`. Header pictures use
/// `"<part>#<relId>"` ids (see `Inline::Image`).
pub fn image_data(pkg: &Package, rel_id: &str) -> Result<Option<(String, Vec<u8>)>> {
    let (source, id) = match rel_id.split_once('#') {
        Some((p, id)) => (PartName::new(p)?, id),
        None => (pkg.main_part()?, rel_id),
    };
    let rels = pkg.relationships(Some(&source))?;
    let Some(rel) = rels.by_id(id) else {
        return Ok(None);
    };
    let Some(target) = rel.resolve(Some(&source)).transpose()? else {
        return Ok(None);
    };
    Ok(pkg.part(&target).map(|b| {
        let mime = pkg
            .content_type(&target)
            .filter(|c| c.starts_with("image/"))
            .unwrap_or(mime_for(&target));
        (mime.to_string(), b.to_vec())
    }))
}

/// Add a picture to the main document part; returns `(relId, cx, cy)` in EMU.
pub fn add_image(pkg: &mut Package, bytes: Vec<u8>) -> Result<(String, u64, u64)> {
    let (w, h, ext, mime) =
        image_info(&bytes).ok_or_else(|| Error::xml("unsupported image format"))?;
    let main = pkg.main_part()?;
    let mut n = 1;
    let part = loop {
        let p = PartName::new(&format!("/word/media/prism{n}.{ext}"))?;
        if !pkg.has_part(&p) {
            break p;
        }
        n += 1;
    };
    if pkg.content_types().default_for(ext).is_none() {
        pkg.content_types_mut().set_default(ext, mime);
    }
    pkg.set_part(part, bytes, None);
    let mut rels = pkg.relationships(Some(&main))?;
    let id = rels.add(
        rel_types::IMAGE,
        &format!("media/prism{n}.{ext}"),
        TargetMode::Internal,
    );
    pkg.set_relationships(Some(&main), &rels);
    let (w, h) = if w == 0 || h == 0 { (400, 300) } else { (w, h) };
    let mut cx = w as u64 * EMU_PER_PX;
    let mut cy = h as u64 * EMU_PER_PX;
    if cx > MAX_CX {
        cy = cy * MAX_CX / cx;
        cx = MAX_CX;
    }
    Ok((id, cx, cy))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmbeddedFont {
    pub name: String,
    /// `regular`, `bold`, `italic` or `boldItalic`.
    pub style: String,
    /// De-obfuscated TrueType/OpenType bytes.
    pub data: Vec<u8>,
}

/// Reverse ECMA-376 §17.8.1 font obfuscation: XOR the first 32 bytes with
/// the 16-byte key derived from the GUID `fontKey` (bytes read in reverse).
pub fn deobfuscate(data: &mut [u8], font_key: &str) -> bool {
    let hex: String = font_key.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() != 32 || data.len() < 32 {
        return false;
    }
    let mut key = [0u8; 16];
    for (i, k) in key.iter_mut().enumerate() {
        let j = 30 - i * 2;
        *k = u8::from_str_radix(&hex[j..j + 2], 16).unwrap_or(0);
    }
    for i in 0..16 {
        data[i] ^= key[i];
        data[i + 16] ^= key[i];
    }
    true
}

/// All fonts embedded in `word/fontTable.xml`.
pub fn embedded_fonts(pkg: &Package) -> Result<Vec<EmbeddedFont>> {
    const FONT_TABLE: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/fontTable";
    let main = pkg.main_part()?;
    let rels = pkg.relationships(Some(&main))?;
    let Some(table) = rels
        .first_of_type(FONT_TABLE)
        .and_then(|r| r.resolve(Some(&main)))
        .transpose()?
    else {
        return Ok(vec![]);
    };
    let Some(xml) = pkg
        .part(&table)
        .map(|b| String::from_utf8_lossy(b).into_owned())
    else {
        return Ok(vec![]);
    };
    let table_rels = pkg.relationships(Some(&table))?;
    let mut out = Vec::new();
    let mut r = Reader::from_str(&xml);
    let mut name = String::new();
    loop {
        match r.read_event().map_err(Error::xml)? {
            Event::Start(e) | Event::Empty(e) => {
                let l = local(&e);
                if l == "font" {
                    name = attr(&e, "name").unwrap_or_default();
                } else if let Some(style) = l.strip_prefix("embed") {
                    let style = match style {
                        "Regular" => "regular",
                        "Bold" => "bold",
                        "Italic" => "italic",
                        "BoldItalic" => "boldItalic",
                        _ => continue,
                    };
                    let (Some(id), Some(key)) = (attr(&e, "id"), attr(&e, "fontKey")) else {
                        continue;
                    };
                    let Some(part) = table_rels
                        .by_id(&id)
                        .and_then(|rel| rel.resolve(Some(&table)))
                        .transpose()?
                    else {
                        continue;
                    };
                    if let Some(bytes) = pkg.part(&part) {
                        let mut data = bytes.to_vec();
                        if deobfuscate(&mut data, &key) {
                            out.push(EmbeddedFont {
                                name: name.clone(),
                                style: style.into(),
                                data,
                            });
                        }
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_headers() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend(640u32.to_be_bytes());
        png.extend(480u32.to_be_bytes());
        assert_eq!(
            image_info(&png).map(|i| (i.0, i.1, i.2)),
            Some((640, 480, "png"))
        );
        let gif = b"GIF89a\x10\x00\x20\x00";
        assert_eq!(image_info(gif).map(|i| (i.0, i.1)), Some((16, 32)));
        let jpeg = [
            0xFF, 0xD8, 0xFF, 0xE0, 0, 4, 0, 0, 0xFF, 0xC0, 0, 11, 8, 0, 50, 0, 100, 3, 0, 0, 0,
        ];
        assert_eq!(image_info(&jpeg).map(|i| (i.0, i.1)), Some((100, 50)));
        assert!(image_info(b"nope").is_none());
    }

    #[test]
    fn deobfuscation_is_an_involution_with_reversed_key() {
        let key = "{0A1B2C3D-4E5F-6071-8293-A4B5C6D7E8F9}";
        let orig: Vec<u8> = (0..40).collect();
        let mut d = orig.clone();
        assert!(deobfuscate(&mut d, key));
        assert_eq!(d[0], 0xF9, "first byte XORs with the last GUID byte");
        assert_eq!(d[16], 16 ^ 0xF9);
        assert_eq!(d[32..], orig[32..]);
        deobfuscate(&mut d, key);
        assert_eq!(d, orig);
    }
}
