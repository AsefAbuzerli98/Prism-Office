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
    /// Footnotes and endnotes (separators excluded).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<Note>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<Comment>,
    /// Headers and footers referenced by the final section.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<HeaderFooter>,
    /// Bibliography sources (Word's `b:Sources` custom XML part).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<Source>,
    pub settings: DocSettings,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DocSettings {
    /// `w:trackRevisions`: record edits as tracked changes.
    pub track_revisions: bool,
    /// `w:autoHyphenation`.
    pub auto_hyphenation: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NoteKind {
    Footnote,
    Endnote,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub kind: NoteKind,
    pub id: i64,
    pub blocks: Vec<Block>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub initials: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    pub blocks: Vec<Block>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HeaderKind {
    Header,
    Footer,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeaderFooter {
    pub kind: HeaderKind,
    /// `default`, `first` or `even`.
    pub which: String,
    pub rel_id: String,
    pub blocks: Vec<Block>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Source {
    /// Unique tag used by CITATION fields.
    pub tag: String,
    /// `Book`, `JournalArticle`, `InternetSite`, `Report`, `DocumentFromInternetSite` …
    pub kind: String,
    pub title: String,
    /// "Last, First" or corporate names.
    pub authors: Vec<String>,
    pub year: String,
    pub publisher: String,
    pub url: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VMerge {
    Restart,
    Continue,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TableCell {
    pub blocks: Vec<Block>,
    /// Grid columns spanned (`w:gridSpan`).
    pub span: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub v_merge: Option<VMerge>,
    /// Twips (`w:tcW type="dxa"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// Background fill `RRGGBB`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shading: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extra: Vec<RawXml>,
}

impl Default for TableCell {
    fn default() -> Self {
        Self {
            blocks: vec![Block::Paragraph(Paragraph::default())],
            span: 1,
            v_merge: None,
            width: None,
            shading: None,
            extra: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TableRow {
    pub cells: Vec<TableCell>,
    /// Repeat as header row (`w:tblHeader`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub header: bool,
    /// Raw `w:trPr` children other than `tblHeader`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub props: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Table {
    /// Grid column widths in twips.
    pub grid: Vec<u32>,
    pub rows: Vec<TableRow>,
    /// Draw cell borders (derived on import; used for new tables on export).
    pub borders: bool,
    /// Raw `w:tblPr` element, preserved verbatim.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub props: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TabAlign {
    Left,
    Center,
    Right,
    Decimal,
    Bar,
    Clear,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabStop {
    /// Twips from the paragraph's left indent origin (page content edge).
    pub pos: i32,
    pub align: TabAlign,
    /// `none`, `dot`, `hyphen`, `underscore`, `middleDot`, `heavy`.
    #[serde(default)]
    pub leader: Option<char>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RevisionKind {
    Insert,
    Delete,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Revision {
    pub kind: RevisionKind,
    pub id: String,
    #[serde(default)]
    pub author: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CommentEdge {
    Start,
    End,
    Reference,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Columns {
    pub count: u32,
    /// Twips between columns.
    pub space: u32,
    #[serde(default)]
    pub separator: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Block {
    Paragraph(Paragraph),
    Table(Table),
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
    /// Right-to-left paragraph (`w:bidi`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub bidi: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tabs: Vec<TabStop>,
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
    /// Right-to-left run (`w:rtl`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rtl: Option<bool>,
    /// Tracked insertion/deletion this run belongs to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<Revision>,
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
            rtl: other.rtl.or(self.rtl),
            revision: None,
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
            bidi: o.bidi || self.bidi,
            tabs: if o.tabs.is_empty() {
                self.tabs.clone()
            } else {
                o.tabs.clone()
            },
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
    /// Inline picture (`wp:inline` with an embedded blip).
    #[serde(rename_all = "camelCase")]
    Image {
        /// Relationship id; header pictures use `"<part>#<relId>"`.
        rel_id: String,
        /// EMU (914400 per inch).
        cx: u64,
        cy: u64,
        #[serde(default)]
        descr: String,
        /// Original run XML (empty for pictures inserted by Prism).
        #[serde(default)]
        xml: String,
    },
    /// Inline vector shape (DrawingML preset geometry), e.g. from ink-to-shape.
    Shape {
        /// `ellipse`, `rect`, `triangle`, `line`, `rightArrow` …
        preset: String,
        cx: u64,
        cy: u64,
        /// Outline colour `RRGGBB`.
        #[serde(default)]
        stroke: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fill: Option<String>,
        #[serde(default)]
        xml: String,
    },
    /// Footnote/endnote reference mark.
    Note {
        kind: NoteKind,
        id: i64,
        #[serde(default)]
        xml: String,
    },
    /// Comment range boundary or reference mark.
    Comment {
        id: String,
        edge: CommentEdge,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub columns: Option<Columns>,
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
            columns: None,
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
                Inline::Image { .. } | Inline::Shape { .. } => s.push('\u{FFFC}'),
                Inline::Opaque { .. } | Inline::Note { .. } | Inline::Comment { .. } => {}
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
                Block::Table(t) => t
                    .rows
                    .iter()
                    .map(|r| {
                        r.cells
                            .iter()
                            .map(|c| {
                                WordDocument {
                                    blocks: c.blocks.clone(),
                                    ..Default::default()
                                }
                                .plain_text()
                            })
                            .collect::<Vec<_>>()
                            .join("\t")
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
                Block::Opaque { label, .. } => format!("[{label}]"),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
