//! Schema element order for `w:pPr`, `w:rPr` and `w:sectPr` (ECMA-376 Part 1
//! §17.3.1.26, §17.3.2.28, §17.6.17). Word rejects out-of-order children, so
//! known and preserved-unknown properties are merged in this order on export.

pub const PPR: &[&str] = &[
    "pStyle",
    "keepNext",
    "keepLines",
    "pageBreakBefore",
    "framePr",
    "widowControl",
    "numPr",
    "suppressLineNumbers",
    "pBdr",
    "shd",
    "tabs",
    "suppressAutoHyphens",
    "kinsoku",
    "wordWrap",
    "overflowPunct",
    "topLinePunct",
    "autoSpaceDE",
    "autoSpaceDN",
    "bidi",
    "adjustRightInd",
    "snapToGrid",
    "spacing",
    "ind",
    "contextualSpacing",
    "mirrorIndents",
    "suppressOverlap",
    "jc",
    "textDirection",
    "textAlignment",
    "textboxTightWrap",
    "outlineLvl",
    "divId",
    "cnfStyle",
    "rPr",
    "sectPr",
    "pPrChange",
];

pub const RPR: &[&str] = &[
    "rStyle",
    "rFonts",
    "b",
    "bCs",
    "i",
    "iCs",
    "caps",
    "smallCaps",
    "strike",
    "dstrike",
    "outline",
    "shadow",
    "emboss",
    "imprint",
    "noProof",
    "snapToGrid",
    "vanish",
    "webHidden",
    "color",
    "spacing",
    "w",
    "kern",
    "position",
    "sz",
    "szCs",
    "highlight",
    "u",
    "effect",
    "bdr",
    "shd",
    "fitText",
    "vertAlign",
    "rtl",
    "cs",
    "em",
    "lang",
    "eastAsianLayout",
    "specVanish",
    "oMath",
    "rPrChange",
];

pub const SECTPR: &[&str] = &[
    "headerReference",
    "footerReference",
    "footnotePr",
    "endnotePr",
    "type",
    "pgSz",
    "pgMar",
    "paperSrc",
    "pgBorders",
    "lnNumType",
    "pgNumType",
    "cols",
    "formProt",
    "vAlign",
    "noEndnote",
    "titlePg",
    "textDirection",
    "bidi",
    "rtlGutter",
    "docGrid",
    "printerSettings",
    "sectPrChange",
];

/// Sort key for a (possibly prefixed) element name within `table`.
pub fn rank(table: &[&str], qname: &str) -> usize {
    let local = qname.rsplit(':').next().unwrap_or(qname);
    // header/footer references form one choice group in CT_SectPr: any order is valid,
    // so they share a rank and keep their original interleaving.
    let local = if local == "footerReference" {
        "headerReference"
    } else {
        local
    };
    table
        .iter()
        .position(|n| *n == local)
        .unwrap_or(table.len())
}

/// Stable-sort (name, xml) fragments by schema order.
pub fn sorted(table: &[&str], mut parts: Vec<(String, String)>) -> String {
    parts.sort_by_key(|(n, _)| rank(table, n));
    parts.into_iter().map(|(_, x)| x).collect()
}
