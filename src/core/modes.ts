/** The three workspace modalities offered by the Application Hub. */
export type Mode = "document" | "workbook" | "presentation";

export interface ModeInfo {
  id: Mode;
  /** Product-facing name. */
  label: string;
  /** Familiar equivalent shown as a hint on the Hub. */
  equivalent: string;
  tagline: string;
  /** Accent palette applied to the window chrome. */
  /** `frame`: the chrome's low-chroma tint of the mode hue (tab strip, ribbon, status). */
  accent: { base: string; strong: string; soft: string; contrast: string; frame: string };
  extensions: readonly string[];
  defaultExtension: string;
  glyph: string;
}

export const MODES: Record<Mode, ModeInfo> = {
  document: {
    id: "document",
    label: "Document",
    equivalent: "Word",
    tagline: "Write, structure and cite with an AI drafting agent.",
    accent: { base: "#2f5fbf", strong: "#244b99", soft: "#e8eef9", contrast: "#ffffff", frame: "#f6f8fb" }, // cobalt
    extensions: ["docx", "docm", "dotx", "dotm"],
    defaultExtension: "docx",
    glyph: "W",
  },
  workbook: {
    id: "workbook",
    label: "Workbook",
    equivalent: "Excel",
    tagline: "Dynamic arrays, regex functions and in-cell Python.",
    accent: { base: "#1e7b4f", strong: "#17633f", soft: "#e4f2ea", contrast: "#ffffff", frame: "#f6f9f7" }, // ledger green
    extensions: ["xlsx", "xlsm", "xltx", "xltm"],
    defaultExtension: "xlsx",
    glyph: "X",
  },
  presentation: {
    id: "presentation",
    label: "Presentation",
    equivalent: "PowerPoint",
    tagline: "Vector slides, Cameo video and a recording studio.",
    accent: { base: "#b4235a", strong: "#921c49", soft: "#f8e6ed", contrast: "#ffffff", frame: "#faf7f8" }, // raspberry
    extensions: ["pptx", "pptm", "potx", "potm", "ppsx", "ppsm"],
    defaultExtension: "pptx",
    glyph: "P",
  },
};

export const MODE_ORDER: readonly Mode[] = ["document", "workbook", "presentation"];

/** Infer the mode from a file name or path, or `null` if unsupported. */
export function modeFromPath(path: string): Mode | null {
  const name = path.split(/[\\/]/).pop() ?? "";
  const dot = name.lastIndexOf(".");
  if (dot <= 0) return null;
  const ext = name.slice(dot + 1).toLowerCase();
  for (const m of MODE_ORDER) {
    if (MODES[m].extensions.includes(ext)) return m;
  }
  return null;
}
