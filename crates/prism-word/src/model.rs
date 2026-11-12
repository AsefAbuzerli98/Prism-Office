use serde::{Deserialize, Serialize};

/// A raw XML element preserved verbatim (unknown property or content).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawXml {
    /// Qualified name, e.g. `w:shd` (used for schema ordering on export).
    pub name: String,
    pub xml: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WordDocument {
    pub blocks: Vec<Block>,
    pub section: Section,
    /// Resolved style catalogue (read-only for the editor; used for rendering).
    pub styles: Vec<StyleInfo>,
    /// Document defaults (`w:docDefaults`).
    pub default_run: Marks,
    pub default_para: ParaFormat,
    /// List definitions available to paragraphs (read-only).
    pub numbering: Vec<NumberingDef>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Block {
    Paragraph(Paragraph),
    /// Unsupported block-level content (tables, content controls …).
    Opaque {
        label: String,
        xml: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Align {
    Left,
    Center,
    Right,
    Justify,
}

impl Align {
    pub(crate) fn from_ooxml(v: &str) -> Option<Self> {
        Some(match v {
            "left" | "start" => Self::Left,
            "center" => Self::Center,
            "right" | "end" => Self::Right,
            "both" | "distribute" => Self::Justify,
            _ => return None,
        })
    }
    pub(crate) fn to_ooxml(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
            Self::Justify => "both",
        }
    }
}

/// Paragraph formatting shared by direct formatting and styles.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ParaFormat {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<Align>,
    /// Twips (1/20 pt).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_before: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_after: Option<u32>,
    /// Line spacing in 240ths of a line (`lineRule="auto"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// Twips.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indent_left: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indent_right: Option<i32>,
    /// Positive = first-line indent, negative = hanging.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indent_first: Option<i32>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub keep_next: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub page_break_before: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outline_level: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListRef {
    pub num_id: u32,
    pub level: u8,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Paragraph {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    #[serde(flatten)]
    pub format: ParaFormat,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<ListRef>,
    /// Paragraph-mark run properties and other unknown `w:pPr` children.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extra: Vec<RawXml>,
    pub inlines: Vec<Inline>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VertAlign {
    Superscript,
    Subscript,
}

/// Character formatting.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Marks {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strike: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    /// Half-points (`w:sz`), e.g. 22 = 11 pt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
    /// `RRGGBB` hex.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlight: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vert_align: Option<VertAlign>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// Hyperlink target (URL or `#bookmark`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extra: Vec<RawXml>,
}

impl Marks {
    /// Overlay `other` on top of `self` (used for style inheritance).
    pub fn overlay(&self, other: &Marks) -> Marks {
        Marks {
            bold: other.bold.or(self.bold),
            italic: other.italic.or(self.italic),
            underline: other.underline.or(self.underline),
            strike: other.strike.or(self.strike),
            font: other.font.clone().or_else(|| self.font.clone()),
            size: other.size.or(self.size),
            color: other.color.clone().or_else(|| self.color.clone()),
            highlight: other.highlight.clone().or_else(|| self.highlight.clone()),
            vert_align: other.vert_align.or(self.vert_align),
            style: other.style.clone().or_else(|| self.style.clone()),
            link: other.link.clone().or_else(|| self.link.clone()),
            extra: Vec::new(),
        }
    }
}

impl ParaFormat {
    pub fn overlay(&self, o: &ParaFormat) -> ParaFormat {
        ParaFormat {
            align: o.align.or(self.align),
            space_before: o.space_before.or(self.space_before),
            space_after: o.space_after.or(self.space_after),
            line: o.line.or(self.line),
            indent_left: o.indent_left.or(self.indent_left),
            indent_right: o.indent_right.or(self.indent_right),
            indent_first: o.indent_first.or(self.indent_first),
            keep_next: o.keep_next || self.keep_next,
            page_break_before: o.page_break_before || self.page_break_before,
            outline_level: o.outline_level.or(self.outline_level),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BreakKind {
    Line,
    Page,
    Column,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Inline {
    Text {
        text: String,
        marks: Marks,
    },
    Tab {
        marks: Marks,
    },
    Break {
        kind: BreakKind,
    },
    /// Unsupported inline content preserved as a complete XML element
    /// (`w:r` with a drawing, a bookmark, a tracked change …). An empty
    /// `label` means the element is invisible (e.g. bookmark markers).
    Opaque {
        label: String,
        xml: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Section {
    /// Twips. A4 portrait = 11906 × 16838.
    pub page_width: u32,
    pub page_height: u32,
    pub margin_top: i32,
    pub margin_right: u32,
    pub margin_bottom: i32,
    pub margin_left: u32,
    pub margin_header: u32,
    pub margin_footer: u32,
    pub landscape: bool,
    /// `w:sectPr` children that precede/follow `pgSz`+`pgMar`, preserved verbatim.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extra: Vec<RawXml>,
}

impl Default for Section {
    fn default() -> Self {
        Self {
            page_width: 11906,
            page_height: 16838,
            margin_top: 1440,
            margin_right: 1440,
            margin_bottom: 1440,
            margin_left: 1440,
            margin_header: 708,
            margin_footer: 708,
            landscape: false,
            extra: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StyleKind {
    Paragraph,
    Character,
    Table,
    Numbering,
}

/// A style with its formatting resolved through the `basedOn` chain.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleInfo {
    pub id: String,
    pub name: String,
    pub kind: StyleKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub based_on: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
    pub is_default: bool,
    pub para: ParaFormat,
    pub run: Marks,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelDef {
    /// `bullet`, `decimal`, `lowerLetter`, `upperRoman` …
    pub format: String,
    /// Level text, e.g. `%1.` or `•`.
    pub text: String,
    pub start: u32,
    /// Twips.
    pub indent_left: i32,
    pub hanging: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NumberingDef {
    pub num_id: u32,
    pub levels: Vec<LevelDef>,
}

/// Reserved list ids Prism uses when the user creates a new list. Their
/// definitions are added to `numbering.xml` on export when referenced.
pub const BULLET_NUM_ID: u32 = 9001;
pub const DECIMAL_NUM_ID: u32 = 9002;

impl Paragraph {
    pub fn plain(text: &str) -> Self {
        Paragraph {
            inlines: if text.is_empty() {
                vec![]
            } else {
                vec![Inline::Text {
                    text: text.into(),
                    marks: Marks::default(),
                }]
            },
            ..Default::default()
        }
    }

    /// Visible text (tabs as `\t`, breaks as `\n`, opaque as U+FFFC).
    pub fn text(&self) -> String {
        let mut s = String::new();
        for i in &self.inlines {
            match i {
                Inline::Text { text, .. } => s.push_str(text),
                Inline::Tab { .. } => s.push('\t'),
                Inline::Break { .. } => s.push('\n'),
                Inline::Opaque { label, .. } if !label.is_empty() => s.push('\u{FFFC}'),
                Inline::Opaque { .. } => {}
            }
        }
        s
    }
}

impl WordDocument {
    /// Plain text of the whole document, one line per paragraph.
    pub fn plain_text(&self) -> String {
        self.blocks
            .iter()
            .map(|b| match b {
                Block::Paragraph(p) => p.text(),
                Block::Opaque { label, .. } => format!("[{label}]"),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
