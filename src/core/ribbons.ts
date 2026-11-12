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
      id: "page-setup",
      label: "Page Setup",
      groups: [
        {
          id: "page",
          label: "Page",
          commands: [
            cmd("doc.margins", "Margins", "▭", 2),
            cmd("doc.orientation", "Orientation", "⟲", 2),
            cmd("doc.pageBreak", "Page Break", "⤓", 2),
            cmd("doc.columns", "Columns", "▥", 2),
          ],
        },
      ],
    },
    {
      id: "citations",
      label: "Citations",
      groups: [
        {
          id: "sources",
          label: "Sources",
          commands: [
            cmd("doc.footnote", "Footnote", "¹", 2),
            cmd("doc.citation", "Citation", "❝", 2),
            cmd("doc.bibliography", "Bibliography", "☰", 2),
          ],
        },
      ],
    },
    {
      id: "structure",
      label: "Document Structure",
      groups: [
        {
          id: "structure",
          label: "Structure",
          commands: [
            cmd("doc.toc", "Contents", "⋮", 2),
            cmd("doc.markdown", "Markdown View", "M↓", 2),
            cmd("doc.accessibility", "Accessibility", "♿", 2),
          ],
        },
        { id: "ai", label: "AI Agent", commands: [cmd("doc.agent", "Draft with Agent", "✦", 2)] },
      ],
    },
  ],
  workbook: [
    FILE_TAB,
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
      id: "design",
      label: "Slide Design",
      groups: [
        {
          id: "slides",
          label: "Slides",
          commands: [
            cmd("ppt.newSlide", "New Slide", "▢", 4),
            cmd("ppt.layout", "Layout", "▤", 4),
            cmd("ppt.theme", "Theme", "◐", 4),
          ],
        },
        { id: "ai", label: "Copilot", commands: [cmd("ppt.generate", "Generate Deck", "✦", 4)] },
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
          ],
        },
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
            cmd("ppt.morph", "Morph", "⇝", 4),
            cmd("ppt.fade", "Fade", "◌", 4),
            cmd("ppt.timeline", "Animation Timeline", "⏱", 4),
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
