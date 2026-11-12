import { MODES, type Mode } from "./modes";

/** Neutral chrome used by the Hub before any modality is chosen. */
const HUB_ACCENT = { base: "#4f46e5", strong: "#4338ca", soft: "#e0e7ff", contrast: "#ffffff" };

/** Apply a modality's accent tokens as CSS custom properties. */
export function applyTheme(mode: Mode | null, root: HTMLElement = document.documentElement) {
  const accent = mode ? MODES[mode].accent : HUB_ACCENT;
  root.style.setProperty("--accent", accent.base);
  root.style.setProperty("--accent-strong", accent.strong);
  root.style.setProperty("--accent-soft", accent.soft);
  root.style.setProperty("--accent-contrast", accent.contrast);
  root.dataset.mode = mode ?? "hub";
}

/** WCAG relative luminance contrast ratio between two `#rrggbb` colours. */
export function contrastRatio(a: string, b: string): number {
  const lum = (hex: string) => {
    const n = parseInt(hex.slice(1), 16);
    const ch = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((v) => {
      const c = v / 255;
      return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
    });
    return 0.2126 * ch[0]! + 0.7152 * ch[1]! + 0.0722 * ch[2]!;
  };
  const [hi, lo] = [lum(a), lum(b)].sort((x, y) => y - x) as [number, number];
  return (hi + 0.05) / (lo + 0.05);
}
