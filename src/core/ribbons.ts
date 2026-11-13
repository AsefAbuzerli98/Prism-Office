import type { Mode } from "./modes";

export interface RibbonCommand {
  id: string;
  label: string;
  /** Short glyph rendered in the button (icon set comes later). */
  icon: string;
  /** Delivery phase in TASKS.md; commands not yet implemented are shown disabled. */
  phase: number;
}

export interface RibbonGroup {
  id: string;
  label: string;
  commands: RibbonCommand[];
}

export interface RibbonTab {
  id: string;
  label: string;
  groups: RibbonGroup[];
}

const cmd = (id: string, label: string, icon: string, phase: number): RibbonCommand => ({
  id,
  label,
  icon,
  phase,
});

/** Commands shared by every modality (file operations live in the host shell). */
const FILE_TAB: RibbonTab = {
  id: "file",
  label: "File",
  groups: [
    {
      id: "file",
      label: "File",
      commands: [
        cmd("file.new", "New", "＋", 1),
        cmd("file.open", "Open", "↥", 1),
        cmd("file.save", "Save", "⤓", 1),
        cmd("file.saveAs", "Save As", "⇣", 1),
      ],
    },
  ],
};

export const RIBBONS: Record<Mode, RibbonTab[]> = {
  document: [
    FILE_TAB,
    {
      id: "typography",
      label: "Typography",
      groups: [
        {
          id: "font",
          label: "Font",
          commands: [
            cmd("doc.bold", "Bold", "B", 2),
            cmd("doc.italic", "Italic", "I", 2),
            cmd("doc.underline", "Underline", "U", 2),
            cmd("doc.strike", "Strikethrough", "S̶", 2),
            cmd("doc.font", "Font", "Aa", 2),
          ],
        },
        {
          id: "paragraph",
          label: "Paragraph",
          commands: [
            cmd("doc.bullets", "Bullets", "•", 2),
            cmd("doc.numbering", "Numbering", "1.", 2),
            cmd("doc.align", "Align", "≡", 2),
          ],
        },
        { id: "styles", label: "Styles", commands: [cmd("doc.styles", "Styles", "¶", 2)] },
      ],
    },
    {
      id: "insert",
      label: "Insert",
      groups: [
        {
          id: "tables",
          label: "Tables",
          commands: [cmd("doc.table", "Table", "▦", 2), cmd("doc.tableTools", "Table Tools", "⊞", 2)],
        },
        {
          id: "illustrations",
          label: "Illustrations",
          commands: [cmd("doc.picture", "Picture", "🖼", 2), cmd("doc.pictureTools", "Picture Tools", "⤢", 2), cmd("doc.ink", "Ink", "✎", 2)],
        },
        {
          id: "header-footer",
          label: "Header & Footer",
          commands: [cmd("doc.header", "Header", "▔", 2), cmd("doc.footer", "Footer", "▁", 2), cmd("doc.pageBreak", "Page Break", "⤓", 2)],
        },
      ],
    },
    {
      id: "page-setup",
      label: "Page Setup",
      groups: [
        {
          id: "page",
          label: "Page",
          commands: [
            cmd("doc.margins", "Margins", "▭", 2),
            cmd("doc.orientation", "Orientation", "⟲", 2),
            cmd("doc.columns", "Columns", "▥", 2),
          ],
        },
        {
          id: "text-flow",
          label: "Text Flow",
          commands: [cmd("doc.hyphenation", "Hyphenation", "‐", 2), cmd("doc.rtl", "Right-to-Left", "⇤", 2)],
        },
      ],
    },
    {
      id: "citations",
      label: "Citations",
      groups: [
        {
          id: "notes",
          label: "Notes",
          commands: [cmd("doc.footnote", "Footnote", "¹", 2), cmd("doc.endnote", "Endnote", "ⁱ", 2)],
        },
        {
          id: "sources",
          label: "Citations",
          commands: [
            cmd("doc.citation", "Citation", "❝", 2),
            cmd("doc.sources", "Sources", "📚", 2),
            cmd("doc.bibliography", "Bibliography", "☰", 2),
          ],
        },
        {
          id: "toc",
          label: "Contents",
          commands: [cmd("doc.toc", "Contents", "⋮", 2), cmd("doc.updateFields", "Update Fields", "↻", 2)],
        },
      ],
    },
    {
      id: "review",
      label: "Review",
      groups: [
        {
          id: "tracking",
          label: "Tracking",
          commands: [cmd("doc.trackChanges", "Track Changes", "✎", 2), cmd("doc.markup", "Show Markup", "◐", 2)],
        },
        {
          id: "changes",
          label: "Changes",
          commands: [
            cmd("doc.accept", "Accept", "✓", 2),
            cmd("doc.reject", "Reject", "✗", 2),
            cmd("doc.acceptAll", "Accept All", "✓✓", 2),
            cmd("doc.rejectAll", "Reject All", "✗✗", 2),
            cmd("doc.nextChange", "Next", "→", 2),
          ],
        },
        {
          id: "comments",
          label: "Comments",
          commands: [cmd("doc.comment", "New Comment", "💬", 2), cmd("doc.comments", "Comments Pane", "🗨", 2)],
        },
        { id: "check", label: "Check", commands: [cmd("doc.accessibility", "Accessibility", "♿", 2)] },
      ],
    },
    {
      id: "structure",
      label: "Document Structure",
      groups: [
        {
          id: "structure",
          label: "Structure",
          commands: [cmd("doc.markdown", "Markdown View", "M↓", 2)],
        },
        {
          id: "ai",
          label: "AI Agent",
          commands: [cmd("doc.agent", "Draft with Agent", "✦", 2), cmd("doc.rewrite", "Rewrite", "✎", 2), cmd("doc.aiSettings", "AI Settings", "⚙", 2)],
        },
      ],
    },
  ],
  workbook: [
    FILE_TAB,
    {
      id: "home",
      label: "Home",
      groups: [
        {
          id: "clipboard",
          label: "Clipboard",
          commands: [cmd("xl.paste", "Paste", "📋", 3), cmd("xl.copy", "Copy", "⧉", 3), cmd("xl.cut", "Cut", "✂", 3), cmd("xl.undo", "Undo", "↶", 3), cmd("xl.redo", "Redo", "↷", 3)],
        },
        {
          id: "font",
          label: "Font",
          commands: [
            cmd("xl.bold", "Bold", "B", 3),
            cmd("xl.italic", "Italic", "I", 3),
            cmd("xl.underline", "Underline", "U", 3),
            cmd("xl.fill", "Fill Color", "▆", 3),
            cmd("xl.color", "Font Color", "A", 3),
            cmd("xl.border", "Borders", "▢", 3),
          ],
        },
        {
          id: "alignment",
          label: "Alignment",
          commands: [cmd("xl.alignLeft", "Left", "⇤", 3), cmd("xl.alignCenter", "Center", "↔", 3), cmd("xl.alignRight", "Right", "⇥", 3), cmd("xl.wrap", "Wrap Text", "↩", 3)],
        },
        { id: "number", label: "Number", commands: [cmd("xl.numfmt", "Number Format", "#", 3), cmd("xl.percent", "Percent", "%", 3)] },
        {
          id: "cells",
          label: "Cells",
          commands: [cmd("xl.autoSum", "AutoSum", "Σ", 3), cmd("xl.freeze", "Freeze Panes", "❄", 3), cmd("xl.addSheet", "New Sheet", "＋", 3)],
        },
      ],
    },
    {
      id: "data",
      label: "Data Connections",
      groups: [
        {
          id: "get-data",
          label: "Get Data",
          commands: [
            cmd("xl.importCsv", "From CSV", "⇲", 3),
            cmd("xl.importSql", "From SQL", "⛁", 3),
            cmd("xl.refresh", "Refresh All", "↻", 3),
          ],
        },
      ],
    },
    {
      id: "formulas",
      label: "Formulas",
      groups: [
        {
          id: "library",
          label: "Function Library",
          commands: [
            cmd("xl.groupby", "GROUPBY", "Σ", 3),
            cmd("xl.pivotby", "PIVOTBY", "⊞", 3),
            cmd("xl.regex", "REGEX", ".*", 3),
            cmd("xl.xlookup", "XLOOKUP", "⌕", 3),
          ],
        },
      ],
    },
    {
      id: "python",
      label: "Python",
      groups: [
        {
          id: "py",
          label: "Python in Cells",
          commands: [
            cmd("xl.py", "Insert =PY", "🐍", 3),
            cmd("xl.pyEnv", "Environment", "⚙", 3),
          ],
        },
      ],
    },
    {
      id: "pivot",
      label: "Pivot Tables",
      groups: [
        {
          id: "pivot",
          label: "Pivot",
          commands: [
            cmd("xl.pivotTable", "PivotTable", "▦", 3),
            cmd("xl.chart", "Chart", "📈", 3),
          ],
        },
      ],
    },
  ],
  presentation: [
    FILE_TAB,
    {
      id: "home",
      label: "Home",
      groups: [
        {
          id: "slides",
          label: "Slides",
          commands: [
            cmd("ppt.newSlide", "New Slide", "▢", 4),
            cmd("ppt.layout", "Layout", "▤", 4),
            cmd("ppt.duplicateSlide", "Duplicate", "⧉", 4),
            cmd("ppt.deleteSlide", "Delete", "✕", 4),
            cmd("ppt.hideSlide", "Hide", "◌", 4),
          ],
        },
        {
          id: "font",
          label: "Font",
          commands: [
            cmd("ppt.font", "Font", "Aa", 4),
            cmd("ppt.bold", "Bold", "B", 4),
            cmd("ppt.italic", "Italic", "I", 4),
            cmd("ppt.underline", "Underline", "U", 4),
            cmd("ppt.strike", "Strikethrough", "S", 4),
            cmd("ppt.grow", "Grow Font", "A↑", 4),
            cmd("ppt.shrink", "Shrink Font", "A↓", 4),
            cmd("ppt.color", "Font Color", "A", 4),
          ],
        },
        {
          id: "paragraph",
          label: "Paragraph",
          commands: [
            cmd("ppt.bullets", "Bullets", "•", 4),
            cmd("ppt.numbering", "Numbering", "1.", 4),
            cmd("ppt.alignLeft", "Left", "⇤", 4),
            cmd("ppt.alignCenter", "Center", "↔", 4),
            cmd("ppt.alignRight", "Right", "⇥", 4),
            cmd("ppt.justify", "Justify", "☰", 4),
          ],
        },
        {
          id: "drawing",
          label: "Drawing",
          commands: [cmd("ppt.fill", "Shape Fill", "▆", 4), cmd("ppt.outline", "Outline", "▢", 4), cmd("ppt.arrange", "Arrange", "⧉", 4)],
        },
      ],
    },
    {
      id: "insert",
      label: "Insert",
      groups: [
        {
          id: "insert",
          label: "Insert",
          commands: [
            cmd("ppt.textBox", "Text Box", "T", 4),
            cmd("ppt.shapes", "Shapes", "◆", 4),
            cmd("ppt.picture", "Picture", "🖼", 4),
            cmd("ppt.table", "Table", "▦", 4),
          ],
        },
      ],
    },
    {
      id: "design",
      label: "Slide Design",
      groups: [
        {
          id: "customize",
          label: "Customize",
          commands: [cmd("ppt.slideSize", "Slide Size", "▭", 4), cmd("ppt.background", "Background", "◐", 4), cmd("ppt.theme", "Theme", "◑", 4)],
        },
        { id: "ai", label: "Copilot", commands: [cmd("ppt.generate", "Generate Deck", "✦", 4)] },
      ],
    },
    {
      id: "transitions",
      label: "Transitions",
      groups: [
        {
          id: "transitions",
          label: "Transitions",
          commands: [
            cmd("ppt.transition", "Transition", "⇢", 4),
            cmd("ppt.morph", "Morph", "⇝", 4),
            cmd("ppt.fade", "Fade", "◌", 4),
          ],
        },
        {
          id: "animations",
          label: "Animations",
          commands: [cmd("ppt.animate", "Add Animation", "✧", 4), cmd("ppt.timeline", "Animation Timeline", "⏱", 4)],
        },
      ],
    },
    {
      id: "slideshow",
      label: "Slide Show",
      groups: [
        {
          id: "start",
          label: "Start",
          commands: [cmd("ppt.showStart", "From Beginning", "▶", 4), cmd("ppt.showCurrent", "From Current", "⏵", 4), cmd("ppt.presenter", "Presenter View", "🖥", 4), cmd("ppt.live", "Present Live", "📡", 4), cmd("ppt.coach", "Rehearse with Coach", "🎙", 4)],
        },
      ],
    },
    {
      id: "cameo",
      label: "Cameo Layouts",
      groups: [
        {
          id: "cameo",
          label: "Cameo",
          commands: [
            cmd("ppt.cameo", "Insert Cameo", "◉", 4),
            cmd("ppt.cameoShape", "Frame Shape", "⬭", 4),
            cmd("ppt.cameoMirror", "Mirror", "⇋", 4),
            cmd("ppt.camera", "Camera", "📷", 4),
          ],
        },
      ],
    },
    {
      id: "recording",
      label: "Recording Studio",
      groups: [
        {
          id: "record",
          label: "Record",
          commands: [
            cmd("ppt.record", "Record", "⏺", 4),
            cmd("ppt.teleprompter", "Teleprompter", "☰", 4),
            cmd("ppt.export", "Export MP4", "🎞", 4),
            cmd("ppt.coach", "Speaker Coach", "🎙", 4),
          ],
        },
      ],
    },
  ],
};

/** Phase whose commands are implemented in the current build. */
export const IMPLEMENTED_PHASE = 1;
