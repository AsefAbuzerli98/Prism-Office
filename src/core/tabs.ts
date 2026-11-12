import type { Mode } from "./modes";

export interface DocTab {
  /** Host-side document id. */
  docId: number;
  mode: Mode;
  title: string;
  path: string | null;
  dirty: boolean;
}

export type TabEvent =
  | { type: "opened"; tab: DocTab }
  | { type: "closed"; tab: DocTab }
  | { type: "activated"; tab: DocTab | null }
  | { type: "updated"; tab: DocTab };

type Listener = (e: TabEvent) => void;

/**
 * Tabbed-MDI model: ordered tabs, one active. Pure state + events so the
 * shell (ribbon, theme, engine view) can react to tab switches.
 */
export class TabManager {
  private tabs: DocTab[] = [];
  private activeId: number | null = null;
  /** Most-recently-used order, used to pick the next active tab on close. */
  private mru: number[] = [];
  private listeners = new Set<Listener>();

  on(listener: Listener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private emit(e: TabEvent) {
    for (const l of this.listeners) l(e);
  }

  list(): readonly DocTab[] {
    return this.tabs;
  }

  get active(): DocTab | null {
    return this.tabs.find((t) => t.docId === this.activeId) ?? null;
  }

  get(docId: number): DocTab | undefined {
    return this.tabs.find((t) => t.docId === docId);
  }

  open(tab: Omit<DocTab, "dirty"> & { dirty?: boolean }): DocTab {
    const existing = this.get(tab.docId);
    if (existing) {
      this.activate(existing.docId);
      return existing;
    }
    const t: DocTab = { dirty: false, ...tab };
    this.tabs.push(t);
    this.emit({ type: "opened", tab: t });
    this.activate(t.docId);
    return t;
  }

  /** Find an open tab for a file path (case-insensitive on separators/case). */
  findByPath(path: string): DocTab | undefined {
    const norm = (p: string) => p.replace(/\\/g, "/").toLowerCase();
    return this.tabs.find((t) => t.path !== null && norm(t.path) === norm(path));
  }

  activate(docId: number | null) {
    if (docId !== null && !this.get(docId)) return;
    if (docId === this.activeId) return;
    this.activeId = docId;
    if (docId !== null) {
      this.mru = [docId, ...this.mru.filter((id) => id !== docId)];
    }
    this.emit({ type: "activated", tab: this.active });
  }

  update(docId: number, patch: Partial<Omit<DocTab, "docId">>) {
    const t = this.get(docId);
    if (!t) return;
    Object.assign(t, patch);
    this.emit({ type: "updated", tab: t });
  }

  close(docId: number): DocTab | undefined {
    const idx = this.tabs.findIndex((t) => t.docId === docId);
    if (idx < 0) return undefined;
    const [tab] = this.tabs.splice(idx, 1);
    this.mru = this.mru.filter((id) => id !== docId);
    this.emit({ type: "closed", tab: tab! });
    if (this.activeId === docId) {
      this.activeId = null;
      this.activate(this.mru[0] ?? null);
      if (this.activeId === null) this.emit({ type: "activated", tab: null });
    }
    return tab;
  }

  /** Cycle to the next/previous tab in strip order. */
  cycle(direction: 1 | -1) {
    if (this.tabs.length === 0) return;
    const idx = this.tabs.findIndex((t) => t.docId === this.activeId);
    const next = (idx + direction + this.tabs.length) % this.tabs.length;
    this.activate(this.tabs[next]!.docId);
  }
}
