/** Command registry, keybindings and fuzzy matching for the command palette. */

export interface Command {
  id: string;
  /** Localised title (evaluated lazily so it follows locale changes). */
  title: () => string;
  /** Keybindings such as "Mod+S", "Ctrl+Shift+Tab". `Mod` = ⌘ on macOS, Ctrl elsewhere. */
  keys?: string[];
  /** Whether the command is currently applicable. */
  when?: () => boolean;
  run: () => void | Promise<void>;
  /** Hide from the palette (still reachable via keys / ribbon). */
  hidden?: boolean;
}

export interface Keybinding {
  mod: boolean;
  ctrl: boolean;
  shift: boolean;
  alt: boolean;
  key: string;
}

export function parseKeybinding(spec: string): Keybinding {
  const kb: Keybinding = { mod: false, ctrl: false, shift: false, alt: false, key: "" };
  for (const raw of spec.split("+")) {
    const p = raw.trim();
    switch (p.toLowerCase()) {
      case "mod":
      case "cmdorctrl":
        kb.mod = true;
        break;
      case "ctrl":
      case "control":
        kb.ctrl = true;
        break;
      case "shift":
        kb.shift = true;
        break;
      case "alt":
      case "option":
        kb.alt = true;
        break;
      default:
        kb.key = p.length === 1 ? p.toLowerCase() : p;
    }
  }
  return kb;
}

export function isMacPlatform(platform: string = navigator.platform): boolean {
  return /mac|iphone|ipad/i.test(platform);
}

export function matchesKeybinding(e: KeyboardEvent, kb: Keybinding, mac: boolean): boolean {
  const wantMeta = kb.mod && mac;
  const wantCtrl = kb.ctrl || (kb.mod && !mac);
  if (e.metaKey !== wantMeta || e.ctrlKey !== wantCtrl) return false;
  if (e.shiftKey !== kb.shift || e.altKey !== kb.alt) return false;
  const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;
  return key === kb.key || (kb.key === "," && e.code === "Comma");
}

export function formatKeybinding(spec: string, mac: boolean): string {
  const kb = parseKeybinding(spec);
  const key = kb.key.length === 1 ? kb.key.toUpperCase() : kb.key;
  if (mac) {
    return `${kb.ctrl ? "⌃" : ""}${kb.alt ? "⌥" : ""}${kb.shift ? "⇧" : ""}${kb.mod ? "⌘" : ""}${key}`;
  }
  return [kb.mod || kb.ctrl ? "Ctrl" : "", kb.alt ? "Alt" : "", kb.shift ? "Shift" : "", key].filter(Boolean).join("+");
}

/**
 * Subsequence fuzzy score: higher is better, `null` when `query` is not a
 * subsequence of `text`. Rewards word-start and consecutive matches.
 */
export function fuzzyScore(query: string, text: string): number | null {
  const q = query.toLowerCase().replace(/\s+/g, "");
  if (!q) return 0;
  const s = text.toLowerCase();
  let score = 0;
  let qi = 0;
  let prev = -2;
  for (let i = 0; i < s.length && qi < q.length; i++) {
    if (s[i] !== q[qi]) continue;
    const wordStart = i === 0 || /[\s\-_./:]/.test(s[i - 1]!);
    score += 1 + (wordStart ? 2 : 0) + (prev === i - 1 ? 3 : 0);
    prev = i;
    qi++;
  }
  if (qi < q.length) return null;
  return score - s.length * 0.01;
}

export class CommandRegistry {
  private commands = new Map<string, Command>();

  constructor(private readonly mac = isMacPlatform()) {}

  get isMac() {
    return this.mac;
  }

  register(...cmds: Command[]) {
    for (const c of cmds) this.commands.set(c.id, c);
  }

  get(id: string) {
    return this.commands.get(id);
  }

  has(id: string) {
    return this.commands.has(id);
  }

  list(): Command[] {
    return [...this.commands.values()];
  }

  isEnabled(id: string): boolean {
    const c = this.commands.get(id);
    return !!c && (c.when?.() ?? true);
  }

  async execute(id: string): Promise<boolean> {
    const c = this.commands.get(id);
    if (!c || !(c.when?.() ?? true)) return false;
    await c.run();
    return true;
  }

  /** Dispatch a keydown; returns true (and prevents default) if handled. */
  handleKeydown(e: KeyboardEvent): boolean {
    for (const c of this.commands.values()) {
      if (!c.keys) continue;
      for (const spec of c.keys) {
        if (matchesKeybinding(e, parseKeybinding(spec), this.mac) && (c.when?.() ?? true)) {
          e.preventDefault();
          void c.run();
          return true;
        }
      }
    }
    return false;
  }

  /** Palette search: enabled, visible commands ranked by fuzzy score. */
  search(query: string): Command[] {
    return this.list()
      .filter((c) => !c.hidden && (c.when?.() ?? true))
      .map((c) => ({ c, s: fuzzyScore(query, c.title()) }))
      .filter((x): x is { c: Command; s: number } => x.s !== null)
      .sort((a, b) => b.s - a.s || a.c.title().localeCompare(b.c.title()))
      .map((x) => x.c);
  }
}
