/**
 * WYSIWYG document editor: canvas rendering, caret/selection, keyboard,
 * IME, clipboard, undo/redo, Markdown split view, and editing inside
 * containers (table cells, footnotes/endnotes, headers/footers), tracked
 * changes, comments, pictures and fields.
 */
import { h } from "../core/dom";
import type { HostBridge } from "../core/host";
import { blocksToHtml, blocksToText, htmlToBlocks, textToBlocks } from "./clipboard";
import { containerBlocks, mapAllBlocks, pruneOrphans, withContainerBlocks } from "./containers";
import { buildBibliography, buildToc, citationField, headings, updateFields } from "./features/fields";
import { newRevision, nextRevision, resolveAll, resolveRange, trackDelete } from "./features/revisions";
import * as tables from "./features/tables";
import { History } from "./history";
import { canvasMeasurer, layoutDocument, sameContainer, type ContainerRef, type Layout, type Line, type Measurer } from "./layout";
import { fromMarkdown, setBulletIds, toMarkdown } from "./markdown";
import {
  BULLET_NUM_ID, DECIMAL_NUM_ID, isParagraph, isTable, newTable, paragraph,
  type Align, type Block, type Inline, type Marks, type NoteKind, type Paragraph, type Revision, type Source, type Table, type WordDocument,
} from "./model";
import {
  applyMarks, blockLength, clampPos, comparePos, deleteRange, insertBlocks, insertInline, insertText, isCollapsed, marksAt,
  ordered, rangeHasMark, sliceBlocks, splitInlines, splitParagraph, stepPos, updateParagraphs, wordBoundary, type Pos, type Selection,
} from "./ops";
import { pageOrigin, paint, totalHeight, type Rect } from "./render";
import { blocksToRtf, rtfToBlocks } from "./rtf";

export interface EngineView {
  readonly el: HTMLElement;
  /** Push pending edits to the host before saving. */
  flush(): Promise<void>;
  /** Run an editor command (ribbon / palette); returns false if unknown. */
  execute(command: string, arg?: string): boolean;
  focus(): void;
  dispose(): void;
}

interface Placed {
  line: Line;
  page: number;
}

interface EdSnapshot {
  doc: WordDocument;
  selection: Selection;
  container: ContainerRef | null;
}

export interface EditorOptions {
  onDirty?: () => void;
  /** Selection or formatting state changed (ribbon refresh). */
  onState?: () => void;
  onChange?: (doc: WordDocument) => void;
  measurer?: Measurer;
  syncDelayMs?: number;
  /** Name recorded on tracked changes and comments. */
  author?: string;
}

const collapsedAt = (p: Pos): Selection => ({ anchor: p, focus: p });

export class WordEditor implements EngineView {
  readonly el: HTMLElement;
  doc: WordDocument | null = null;
  selection: Selection = collapsedAt({ block: 0, offset: 0 });
  /** Container being edited; null = document body. */
  container: ContainerRef | null = null;
  readonly history = new History<EdSnapshot>();
  showMarkup = true;
  layout: Layout | null = null;
  private allLines: Placed[] = [];
  private lines: Placed[] = [];
  private readonly scroller: HTMLElement;
  private readonly spacer: HTMLElement;
  private readonly canvas: HTMLCanvasElement;
  private readonly input: HTMLTextAreaElement;
  private readonly mdPane: HTMLTextAreaElement;
  readonly side: HTMLElement;
  private readonly measurer: Measurer;
  private pendingMarks: Marks | null = null;
  private typingRevision: Revision | null = null;
  private goalX: number | null = null;
  private composing = false;
  private dragging = false;
  private caretOn = true;
  private blink: ReturnType<typeof setInterval> | null = null;
  private syncTimer: ReturnType<typeof setTimeout> | null = null;
  private syncing: Promise<void> = Promise.resolve();
  private mdTimer: ReturnType<typeof setTimeout> | null = null;
  private mdObjects = new Map<string, Block | Inline>();
  private images = new Map<string, HTMLImageElement | "loading" | "failed">();
  private dirty = false;
  private raf = 0;
  private resizeObs: ResizeObserver | null = null;
  readonly ready: Promise<void>;
  readonly author: string;

  constructor(
    private readonly host: HostBridge,
    readonly docId: number,
    private readonly opts: EditorOptions = {},
  ) {
    this.author = opts.author ?? "Prism User";
    this.measurer = opts.measurer ?? canvasMeasurer();
    this.canvas = h("canvas", { class: "word-canvas", "aria-hidden": "true" });
    this.spacer = h("div", { class: "word-spacer" });
    this.input = h("textarea", { class: "word-input", "aria-label": "Document text", autocapitalize: "off", autocomplete: "off", spellcheck: "false" });
    this.scroller = h("div", { class: "word-scroll", tabindex: "-1" }, this.canvas, this.spacer, this.input);
    this.mdPane = h("textarea", { class: "word-md", "aria-label": "Markdown source", spellcheck: "false", hidden: true });
    this.side = h("aside", { class: "word-side", hidden: true });
    this.el = h("div", { class: "word-editor" }, this.scroller, this.mdPane, this.side);
    this.bind();
    this.ready = this.load();
  }

  // ---- lifecycle -------------------------------------------------------------

  private async load() {
    const doc = await this.host.getWordDocument(this.docId);
    this.doc = doc;
    setBulletIds(doc.numbering.filter((n) => n.levels[0]?.format === "bullet").map((n) => n.numId));
    this.selection = collapsedAt({ block: 0, offset: 0 });
    this.relayout();
    void this.loadFonts();
  }

  /** Register fonts embedded in the DOCX with the FontFace API, then re-layout. */
  private async loadFonts() {
    if (typeof FontFace === "undefined" || !document.fonts) return;
    try {
      const fonts = await this.host.getWordFonts(this.docId);
      for (const f of fonts) {
        const bin = Uint8Array.from(atob(f.data), (c) => c.charCodeAt(0));
        const face = new FontFace(f.name, bin, {
          weight: f.style.startsWith("bold") ? "700" : "400",
          style: f.style.toLowerCase().includes("italic") ? "italic" : "normal",
        });
        await face.load();
        document.fonts.add(face);
      }
      if (fonts.length) this.relayout();
    } catch {
      /* fonts are optional */
    }
  }

  focus() {
    this.input.focus({ preventScroll: true });
  }

  dispose() {
    if (this.blink) clearInterval(this.blink);
    this.resizeObs?.disconnect();
    cancelAnimationFrame(this.raf);
  }

  async flush() {
    if (this.syncTimer) {
      clearTimeout(this.syncTimer);
      this.syncTimer = null;
      this.syncing = this.pushToHost();
    }
    await this.syncing;
  }

  private async pushToHost() {
    if (this.doc) await this.host.updateWordDocument(this.docId, this.doc);
  }

  // ---- state changes ---------------------------------------------------------

  /** Blocks of the container being edited. */
  get blocks(): Block[] {
    return this.doc ? containerBlocks(this.doc, this.container) : [];
  }

  private snapshot(): EdSnapshot {
    return { doc: this.doc!, selection: this.selection, container: this.container };
  }

  /** Apply new blocks for the current container as one undoable step. */
  commit(kind: string, blocks: Block[], selection: Selection) {
    if (!this.doc) return;
    this.commitDoc(kind, withContainerBlocks(this.doc, this.container, blocks), selection);
  }

  /** Apply a whole-document change as one undoable step. */
  commitDoc(kind: string, doc: WordDocument, selection: Selection = this.selection, container: ContainerRef | null = this.container) {
    if (!this.doc) return;
    this.history.record(this.snapshot(), kind);
    this.doc = pruneOrphans(doc);
    this.container = containerBlocks(this.doc, container).length || !container ? container : null;
    const blocks = this.blocks;
    this.selection = { anchor: clampPos(blocks, selection.anchor), focus: clampPos(blocks, selection.focus) };
    this.pendingMarks = null;
    this.goalX = null;
    this.changed();
  }

  private changed() {
    if (!this.dirty) {
      this.dirty = true;
      this.opts.onDirty?.();
    }
    if (this.syncTimer) clearTimeout(this.syncTimer);
    this.syncTimer = setTimeout(() => {
      this.syncTimer = null;
      this.syncing = this.pushToHost();
    }, this.opts.syncDelayMs ?? 400);
    this.relayout();
    if (!this.mdPane.hidden && document.activeElement !== this.mdPane) this.refreshMarkdown();
    this.refreshSide();
    this.opts.onChange?.(this.doc!);
    this.opts.onState?.();
  }

  /** Mark saved: next edit fires onDirty again. */
  markClean() {
    this.dirty = false;
  }

  setSelection(sel: Selection, keepGoal = false) {
    this.selection = sel;
    if (!keepGoal) this.goalX = null;
    this.pendingMarks = null;
    this.typingRevision = null;
    this.history.seal();
    this.caretOn = true;
    this.scheduleRender();
    this.scrollCaretIntoView();
    this.opts.onState?.();
  }

  /** Switch the edited container and place the caret. */
  setContainer(c: ContainerRef | null, pos: Pos = { block: 0, offset: 0 }) {
    this.container = c;
    this.filterLines();
    this.setSelection(collapsedAt(clampPos(this.blocks, pos)));
    this.refreshSide();
  }

  // ---- layout & paint ----------------------------------------------------------

  private relayout() {
    if (!this.doc) return;
    this.layout = layoutDocument(this.doc, this.measurer, { showMarkup: this.showMarkup });
    this.allLines = this.layout.pages.flatMap((p) => p.lines.map((line) => ({ line, page: p.index })));
    this.filterLines();
    this.spacer.style.height = `${totalHeight(this.layout)}px`;
    this.spacer.style.width = `${this.layout.width + 48}px`;
    this.scheduleRender();
  }

  private filterLines() {
    this.lines = this.allLines.filter((l) => sameContainer(l.line.container, this.container));
  }

  private scheduleRender() {
    if (this.raf) return;
    this.raf = requestAnimationFrame(() => {
      this.raf = 0;
      this.render();
    });
  }

  private image(relId: string): CanvasImageSource | null {
    const hit = this.images.get(relId);
    if (hit instanceof HTMLImageElement) return hit.complete ? hit : null;
    if (hit) return null;
    this.images.set(relId, "loading");
    void this.host.getWordImage(this.docId, relId).then(
      (d) => {
        if (!d) return void this.images.set(relId, "failed");
        const img = new Image();
        img.onload = () => this.scheduleRender();
        img.src = `data:${d.mime};base64,${d.data}`;
        this.images.set(relId, img);
      },
      () => this.images.set(relId, "failed"),
    );
    return null;
  }

  render() {
    if (!this.layout) return;
    const vp = this.viewport();
    if (vp.width === 0 || vp.height === 0) return;
    const dpr = window.devicePixelRatio || 1;
    const W = Math.round(vp.width * dpr);
    const H = Math.round(vp.height * dpr);
    if (this.canvas.width !== W || this.canvas.height !== H) {
      this.canvas.width = W;
      this.canvas.height = H;
      this.canvas.style.width = `${vp.width}px`;
      this.canvas.style.height = `${vp.height}px`;
    }
    this.canvas.style.transform = `translate(${vp.scrollLeft}px, ${vp.scrollTop}px)`;
    const ctx = this.canvas.getContext("2d");
    if (!ctx) return;
    const caret = isCollapsed(this.selection) && this.caretOn && document.activeElement === this.input ? this.caretRect(this.selection.focus) : null;
    paint(ctx, this.layout, vp, dpr, this.selectionRects(), caret, document.documentElement.dataset.appearance === "dark", {
      active: this.container,
      images: (id) => this.image(id),
    });
    this.positionInput();
  }

  private viewport() {
    return { scrollTop: this.scroller.scrollTop, scrollLeft: this.scroller.scrollLeft, width: this.scroller.clientWidth, height: this.scroller.clientHeight };
  }

  private vpw() {
    return this.scroller.clientWidth || (this.layout?.width ?? 800) + 48;
  }

  // ---- geometry ------------------------------------------------------------------

  private lineIndexOf(p: Pos): number {
    let fallback = -1;
    for (let i = 0; i < this.lines.length; i++) {
      const { line } = this.lines[i]!;
      if (line.block !== p.block) {
        if (fallback >= 0) break;
        continue;
      }
      fallback = i;
      if (p.offset >= line.start && (p.offset < line.end || (p.offset === line.end && (line.last || line.end === line.start)))) return i;
    }
    return fallback;
  }

  caretRect(p: Pos): Rect | null {
    if (!this.layout) return null;
    const idx = this.lineIndexOf(p);
    if (idx < 0) {
      for (const page of this.layout.pages) {
        const o = pageOrigin(this.layout, page.index, this.vpw());
        if (!this.container) {
          const box = page.boxes.find((b) => b.block === p.block);
          if (box) return { x: o.x + (p.offset ? box.right : box.left) - 2, y: o.y + box.y, w: 2, h: box.height };
          const cells = page.areas.filter((a) => a.container.type === "cell" && a.container.table === p.block);
          if (cells.length) {
            const a = p.offset ? cells[cells.length - 1]! : cells[0]!;
            return { x: o.x + a.x - 3, y: o.y + (p.offset ? a.y + a.h - 16 : a.y), w: 2, h: 16 };
          }
        }
      }
      return null;
    }
    const { line, page } = this.lines[idx]!;
    const o = pageOrigin(this.layout, page, this.vpw());
    return { x: o.x + this.xInLine(line, p.offset), y: o.y + line.y, w: 2, h: line.height };
  }

  private charX(f: Line["frags"][number], k: number): number {
    const w = this.measurer.width(f.text.slice(0, k), f.style.font);
    return f.rtl ? f.x + f.width - w : f.x + w;
  }

  private xInLine(line: Line, offset: number): number {
    const frags = line.frags.filter((f) => f.kind !== "marker" && f.kind !== "hyphen");
    for (const f of frags) {
      if (offset >= f.start && offset <= f.end && (offset < f.end || f === frags[frags.length - 1] || f.kind !== "text")) {
        if (f.kind !== "text") return offset === f.start ? f.x : f.x + f.width;
        return this.charX(f, offset - f.start);
      }
    }
    const last = frags[frags.length - 1];
    return last ? last.x + last.width : line.left;
  }

  /** Map a point (scroller content coordinates) to a position in the current container. */
  hitTest(x: number, y: number): Pos {
    if (!this.layout || this.lines.length === 0) return { block: 0, offset: 0 };
    const pageIdx = Math.max(0, Math.min(this.layout.pages.length - 1, Math.floor((y - 12) / (this.layout.height + 24))));
    const o = pageOrigin(this.layout, pageIdx, this.vpw());
    const px = x - o.x;
    const py = y - o.y;
    const page = this.layout.pages[pageIdx]!;
    if (!this.container) {
      for (const b of page.boxes) if (b.block >= 0 && py >= b.y && py <= b.y + b.height) return { block: b.block, offset: px > (b.left + b.right) / 2 ? 1 : 0 };
    }
    let cands = this.lines.filter((l) => l.page === pageIdx).map((l) => l.line);
    if (!cands.length) {
      const before = this.lines.filter((l) => l.page < pageIdx);
      cands = [(before[before.length - 1] ?? this.lines[0]!).line];
    }
    let best = cands[0]!;
    let bestD = Infinity;
    for (const line of cands) {
      const dy = py < line.y ? line.y - py : py > line.y + line.height ? py - line.y - line.height : 0;
      const dx = px < line.left - 20 ? line.left - 20 - px : px > line.right + 20 ? px - line.right - 20 : 0;
      const d = dy * 4 + dx;
      if (d < bestD) {
        bestD = d;
        best = line;
      }
    }
    return { block: best.block, offset: this.offsetInLine(best, px) };
  }

  /** Container + position under a point; headers only when allowed or active. */
  hitTarget(x: number, y: number, allowHeader: boolean): { container: ContainerRef | null; pos: Pos } {
    if (!this.layout) return { container: null, pos: { block: 0, offset: 0 } };
    const pageIdx = Math.max(0, Math.min(this.layout.pages.length - 1, Math.floor((y - 12) / (this.layout.height + 24))));
    const o = pageOrigin(this.layout, pageIdx, this.vpw());
    const px = x - o.x;
    const py = y - o.y;
    const area = this.layout.pages[pageIdx]!.areas.find((a) => px >= a.x - 4 && px <= a.x + a.w + 4 && py >= a.y - 4 && py <= a.y + a.h + 4);
    let c: ContainerRef | null = null;
    if (area && (area.container.type !== "header" || allowHeader || sameContainer(area.container, this.container))) c = area.container;
    const prev = this.container;
    this.container = c;
    this.filterLines();
    const pos = this.hitTest(x, y);
    this.container = prev;
    this.filterLines();
    return { container: c, pos };
  }

  private offsetInLine(line: Line, px: number): number {
    const frags = line.frags.filter((f) => f.kind !== "marker" && f.kind !== "hyphen").slice().sort((a, b) => a.x - b.x);
    for (const f of frags) {
      if (px < f.x + f.width) {
        if (px <= f.x) return f.rtl ? f.end : f.start;
        if (f.kind !== "text") return px > f.x + f.width / 2 ? f.end : f.start;
        let best = f.start;
        let bestD = Infinity;
        for (let i = 0; i <= f.text.length; i++) {
          const d = Math.abs(this.charX(f, i) - px);
          if (d < bestD) {
            bestD = d;
            best = f.start + i;
          }
        }
        return best;
      }
    }
    const last = line.frags.filter((f) => f.kind !== "marker" && f.kind !== "hyphen").pop();
    if (!line.last && last?.kind === "text" && /\s$/.test(last.text)) return line.end - 1;
    return line.end;
  }

  private selectionRects(): Rect[] {
    if (!this.layout || isCollapsed(this.selection)) return [];
    const [a, b] = ordered(this.selection);
    const rects: Rect[] = [];
    for (const { line, page } of this.lines) {
      if (line.block < a.block || line.block > b.block) continue;
      const s = line.block === a.block ? Math.max(a.offset, line.start) : line.start;
      const e = line.block === b.block ? Math.min(b.offset, line.end) : line.end;
      if (s > e || (s === e && !(line.block < b.block && line.last))) continue;
      const o = pageOrigin(this.layout, page, this.vpw());
      const x1 = this.xInLine(line, s);
      let x2 = this.xInLine(line, e);
      if (line.last && line.block < b.block) x2 += 6;
      rects.push({ x: o.x + Math.min(x1, x2), y: o.y + line.y, w: Math.max(2, Math.abs(x2 - x1)), h: line.height });
    }
    if (!this.container) {
      for (const page of this.layout.pages) {
        const o = pageOrigin(this.layout, page.index, this.vpw());
        for (const box of page.boxes) {
          if (box.block >= 0 && comparePos({ block: box.block, offset: 0 }, a) >= 0 && comparePos({ block: box.block, offset: 1 }, b) <= 0) {
            rects.push({ x: o.x + box.left, y: o.y + box.y, w: box.right - box.left, h: box.height });
          }
        }
        for (const ar of page.areas) {
          if (ar.container.type === "cell" && comparePos({ block: ar.container.table, offset: 0 }, a) >= 0 && comparePos({ block: ar.container.table, offset: 1 }, b) <= 0) {
            rects.push({ x: o.x + ar.x, y: o.y + ar.y, w: ar.w, h: ar.h });
          }
        }
      }
    }
    return rects;
  }

  private positionInput() {
    const r = this.caretRect(this.selection.focus);
    if (!r) return;
    this.input.style.transform = `translate(${r.x}px, ${r.y}px)`;
    this.input.style.height = `${r.h}px`;
  }

  private scrollCaretIntoView() {
    const r = this.caretRect(this.selection.focus);
    if (!r) return;
    const s = this.scroller;
    if (r.y < s.scrollTop + 8) s.scrollTop = Math.max(0, r.y - 40);
    else if (r.y + r.h > s.scrollTop + s.clientHeight - 8) s.scrollTop = r.y + r.h - s.clientHeight + 40;
  }

  // ---- movement ------------------------------------------------------------------

  private moveVertical(p: Pos, dir: 1 | -1, lines = 1): Pos {
    const cur = this.caretRect(p);
    if (!cur || !this.layout) return p;
    const x = this.goalX ?? cur.x;
    this.goalX = x;
    let idx = this.lineIndexOf(p);
    if (idx < 0) {
      idx = dir > 0 ? this.lines.findIndex((l) => l.line.block > p.block) : this.lines.map((l) => l.line.block < p.block).lastIndexOf(true);
      if (idx < 0) return dir > 0 ? clampPos(this.blocks, { block: this.blocks.length - 1, offset: Infinity }) : { block: 0, offset: 0 };
      lines -= 1;
    }
    const target = Math.max(0, Math.min(this.lines.length - 1, idx + dir * lines));
    const between = this.blocks.findIndex((b, bi) => !isParagraph(b) && (dir > 0 ? bi > p.block && bi < this.lines[target]!.line.block : bi < p.block && bi > this.lines[target]!.line.block));
    if (lines === 1 && between >= 0) return { block: between, offset: 0 };
    if (target === idx) return dir > 0 ? { block: p.block, offset: this.lines[idx]!.line.end } : { block: p.block, offset: this.lines[idx]!.line.start };
    const { line, page } = this.lines[target]!;
    const o = pageOrigin(this.layout, page, this.vpw());
    return { block: line.block, offset: this.offsetInLine(line, x - o.x) };
  }

  private isFirstLine(p: Pos) {
    return this.lineIndexOf(p) <= 0;
  }
  private isLastLine(p: Pos) {
    const i = this.lineIndexOf(p);
    return i < 0 || i === this.lines.length - 1;
  }

  private lineEdge(p: Pos, end: boolean): Pos {
    const idx = this.lineIndexOf(p);
    if (idx < 0) return { block: p.block, offset: end ? 1 : 0 };
    const line = this.lines[idx]!.line;
    if (!end) return { block: p.block, offset: line.start };
    const last = line.frags[line.frags.length - 1];
    const off = !line.last && last?.kind === "text" && /\s$/.test(last.text) ? line.end - 1 : line.end;
    return { block: p.block, offset: off };
  }

  private table(): { t: Table; c: Extract<ContainerRef, { type: "cell" }> } | null {
    const c = this.container;
    if (!this.doc || c?.type !== "cell") return null;
    const t = this.doc.blocks[c.table];
    return t && isTable(t) ? { t, c } : null;
  }

  /** Enter a table from the body at its first or last cell. */
  private enterTable(index: number, fromEnd: boolean) {
    const t = this.doc!.blocks[index] as Table;
    const row = fromEnd ? t.rows.length - 1 : 0;
    const col = fromEnd ? t.rows[row]!.cells.length - 1 : 0;
    const blocks = t.rows[row]!.cells[col]!.blocks;
    this.setContainer({ type: "cell", table: index, row, col }, fromEnd ? { block: blocks.length - 1, offset: Infinity } : { block: 0, offset: 0 });
  }

  /** Leave the current container towards `dir` in the body. */
  private exitContainer(dir: 1 | -1) {
    const c = this.container;
    if (!c || !this.doc) return;
    if (c.type === "cell") {
      const target = c.table + dir;
      if (target < 0) return this.setContainer(null, { block: 0, offset: 0 });
      if (target >= this.doc.blocks.length) {
        const next = [...this.doc.blocks, paragraph()];
        this.container = null;
        this.commitDoc("insert", { ...this.doc, blocks: next }, collapsedAt({ block: next.length - 1, offset: 0 }), null);
        this.filterLines();
        return;
      }
      const tb = this.doc.blocks[target]!;
      if (isTable(tb)) return this.enterTable(target, dir < 0);
      return this.setContainer(null, dir > 0 ? { block: target, offset: 0 } : { block: target, offset: blockLength(tb) });
    }
    if (c.type === "note") {
      // caret back to just after the reference mark
      let found: Pos | null = null;
      this.doc.blocks.forEach((b, bi) => {
        if (!isParagraph(b) || found) return;
        let at = 0;
        for (const i of b.inlines) {
          if (i.type === "note" && i.kind === c.kind && i.id === c.id) found = { block: bi, offset: at + 1 };
          at += i.type === "text" ? i.text.length : 1;
        }
      });
      return this.setContainer(null, found ?? { block: 0, offset: 0 });
    }
    this.setContainer(null, { block: 0, offset: 0 });
  }

  private moveCell(dir: 1 | -1, vertical: boolean, addRow = false): boolean {
    const tc = this.table();
    if (!tc) return false;
    const { t, c } = tc;
    let { row, col } = c;
    if (vertical) {
      row += dir;
      if (row < 0 || row >= t.rows.length) {
        this.exitContainer(dir);
        return true;
      }
      col = Math.min(col, t.rows[row]!.cells.length - 1);
    } else {
      col += dir;
      if (col >= t.rows[row]!.cells.length) {
        row++;
        col = 0;
      } else if (col < 0) {
        row--;
        col = row >= 0 ? t.rows[row]!.cells.length - 1 : 0;
      }
      if (row >= t.rows.length) {
        if (!addRow) {
          this.exitContainer(1);
          return true;
        }
        const nt = tables.insertRow(t, t.rows.length - 1, true);
        const next = this.doc!.blocks.slice();
        next[c.table] = nt;
        const cont: ContainerRef = { type: "cell", table: c.table, row, col: 0 };
        this.commitDoc("table", { ...this.doc!, blocks: next }, collapsedAt({ block: 0, offset: 0 }), cont);
        this.filterLines();
        return true;
      }
      if (row < 0) {
        this.exitContainer(-1);
        return true;
      }
    }
    const blocks = t.rows[row]!.cells[col]!.blocks;
    const pos = dir > 0 || vertical ? { block: 0, offset: 0 } : { block: blocks.length - 1, offset: Infinity };
    this.setContainer({ type: "cell", table: c.table, row, col }, pos);
    return true;
  }

  // ---- editing primitives -------------------------------------------------------------

  private nextStyleOf = (style: string | undefined) => this.doc?.styles.find((x) => x.id === style)?.next;

  private get tracking() {
    return !!this.doc?.settings?.trackRevisions;
  }

  private typingMarks(): Marks {
    let m: Marks;
    if (this.pendingMarks) m = this.pendingMarks;
    else {
      const p = this.blocks[this.selection.focus.block];
      const { link: _link, revision: _rev, ...rest } = p && isParagraph(p) ? marksAt(p, this.selection.focus.offset) : ({} as Marks);
      m = rest;
    }
    if (this.tracking) {
      this.typingRevision ??= newRevision("insert", this.author);
      return { ...m, revision: this.typingRevision };
    }
    const { revision: _r, ...rest } = m;
    return rest;
  }

  private removeRange(a: Pos, b: Pos): { blocks: Block[]; pos: Pos } {
    if (!this.tracking || comparePos(a, b) === 0) return deleteRange(this.blocks, a, b);
    return { blocks: trackDelete(this.blocks, a, b, newRevision("delete", this.author)), pos: a };
  }

  /** Replace the selection with text. */
  insertText(text: string, kind = "type") {
    if (!this.doc) return;
    const marks = this.typingMarks();
    const [a, b] = ordered(this.selection);
    let { blocks, pos } = this.removeRange(a, b);
    if (this.tracking && comparePos(a, b) !== 0) pos = b.block === a.block ? { block: a.block, offset: a.offset + (blockLength(blocks[a.block]!) - blockLength(this.blocks[a.block]!)) + (b.offset - a.offset) } : b;
    ({ blocks, pos } = insertText(blocks, pos, text, marks, this.nextStyleOf));
    const keep = this.typingRevision;
    this.commit(kind, blocks, collapsedAt(pos));
    this.typingRevision = keep;
  }

  private insertBlocksAtSelection(frag: Block[]) {
    const [a, b] = ordered(this.selection);
    let { blocks, pos } = deleteRange(this.blocks, a, b);
    ({ blocks, pos } = insertBlocks(blocks, pos, frag));
    this.commit("paste", blocks, collapsedAt(pos));
  }

  insertInlineAtCaret(inline: Inline, kind = "insert") {
    const [a, b] = ordered(this.selection);
    let r = deleteRange(this.blocks, a, b);
    r = insertInline(r.blocks, r.pos, inline);
    this.commit(kind, r.blocks, collapsedAt(r.pos));
  }

  deleteSelectionOr(dir: 1 | -1, word = false) {
    if (!isCollapsed(this.selection)) {
      const [a, b] = ordered(this.selection);
      const before = blockLength(this.blocks[b.block] ?? paragraph());
      const r = this.removeRange(a, b);
      const removed = blockLength(r.blocks[a.block] ?? paragraph()) < before;
      this.commit("delete", r.blocks, collapsedAt(this.tracking && dir > 0 && !removed ? b : r.pos));
      return;
    }
    const p = this.selection.focus;
    const blk = this.blocks[p.block];
    if (dir < 0 && p.offset === 0 && blk && isParagraph(blk) && (blk.list || (blk.style && blk.style !== "Normal" && blockLength(blk) === 0))) {
      this.commit("format", updateParagraphs(this.blocks, p, p, (q) => (q.list ? { ...q, list: undefined } : { ...q, style: undefined })), this.selection);
      return;
    }
    const q = word ? wordBoundary(this.blocks, p, dir) : stepPos(this.blocks, p, dir);
    if (comparePos(p, q) === 0) return;
    if (this.tracking) {
      const [a, b] = comparePos(p, q) < 0 ? [p, q] : [q, p];
      if (a.block !== b.block) return this.setSelection(collapsedAt(q));
      const before = blockLength(this.blocks[a.block]!);
      const blocks = trackDelete(this.blocks, a, b, newRevision("delete", this.author));
      const removed = blockLength(blocks[a.block]!) < before;
      this.commit("delete", blocks, collapsedAt(dir < 0 ? a : removed ? a : b));
      return;
    }
    const r = deleteRange(this.blocks, p, q);
    this.commit("delete", r.blocks, collapsedAt(r.pos));
  }

  toggleMark(mark: "bold" | "italic" | "underline" | "strike") {
    if (isCollapsed(this.selection)) {
      const m = { ...(this.pendingMarks ?? this.typingMarks()) };
      delete m.revision;
      if (m[mark]) delete m[mark];
      else m[mark] = true;
      this.pendingMarks = m;
      return;
    }
    const [a, b] = ordered(this.selection);
    const on = rangeHasMark(this.blocks, a, b, mark);
    this.commit("format", applyMarks(this.blocks, a, b, { [mark]: on ? undefined : true }), this.selection);
  }

  setMarks(patch: Partial<Marks>) {
    if (isCollapsed(this.selection)) {
      const { revision: _r, ...m } = this.typingMarks();
      this.pendingMarks = { ...m, ...patch };
      return;
    }
    const [a, b] = ordered(this.selection);
    this.commit("format", applyMarks(this.blocks, a, b, patch), this.selection);
  }

  updateParagraphs(fn: (p: Paragraph) => Paragraph) {
    const [a, b] = ordered(this.selection);
    this.commit("format", updateParagraphs(this.blocks, a, b, fn), this.selection);
  }

  toggleList(numId: number) {
    const [a] = ordered(this.selection);
    const first = this.blocks[a.block];
    const off = first && isParagraph(first) && first.list?.numId === numId;
    this.updateParagraphs((p) => ({ ...p, list: off ? undefined : { numId, level: p.list?.level ?? 0 } }));
  }

  indent(dir: 1 | -1) {
    this.updateParagraphs((p) => {
      if (p.list) return { ...p, list: { ...p.list, level: Math.max(0, Math.min(8, p.list.level + dir)) } };
      const v = Math.max(0, (p.indentLeft ?? 0) + dir * 720);
      return { ...p, indentLeft: v || undefined };
    });
  }

  undo() {
    if (!this.doc) return;
    const s = this.history.undo(this.snapshot());
    if (s) this.restore(s);
  }

  redo() {
    if (!this.doc) return;
    const s = this.history.redo(this.snapshot());
    if (s) this.restore(s);
  }

  private restore(s: EdSnapshot) {
    this.doc = s.doc;
    this.container = s.container;
    this.selection = s.selection;
    this.changed();
  }

  selectAll() {
    const last = this.blocks.length - 1;
    this.setSelection({ anchor: { block: 0, offset: 0 }, focus: { block: last, offset: blockLength(this.blocks[last]!) } });
  }

  /** The picture/shape covered by the selection, if exactly one object is selected. */
  selectedObject(): { pos: Pos; inline: Extract<Inline, { type: "image" | "shape" }> } | null {
    const [a, b] = ordered(this.selection);
    if (a.block !== b.block || b.offset - a.offset !== 1) return null;
    const p = this.blocks[a.block];
    if (!p || !isParagraph(p)) return null;
    const [, rest] = splitInlines(p.inlines, a.offset);
    const i = rest[0];
    return i && (i.type === "image" || i.type === "shape") ? { pos: a, inline: i } : null;
  }

  private replaceObject(fn: (i: Extract<Inline, { type: "image" | "shape" }>) => Inline) {
    const sel = this.selectedObject();
    if (!sel) return false;
    const p = this.blocks[sel.pos.block] as Paragraph;
    const [left, rest] = splitInlines(p.inlines, sel.pos.offset);
    const next = this.blocks.slice();
    next[sel.pos.block] = { ...p, inlines: [...left, fn(sel.inline), ...rest.slice(1)] };
    this.commit("format", next, this.selection);
    return true;
  }

  // ---- feature commands ------------------------------------------------------------------

  async insertImage(base64?: string | null) {
    const data = base64 ?? (await this.host.pickImage());
    if (!data || !this.doc) return;
    const info = await this.host.addWordImage(this.docId, data);
    this.insertInlineAtCaret({ type: "image", relId: info.relId, cx: info.cx, cy: info.cy, descr: "", xml: "" });
  }

  insertTable(rows: number, cols: number) {
    if (!this.doc) return;
    if (this.container) this.setContainer(null, { block: 0, offset: 0 });
    const s = this.doc.section;
    const width = s.pageWidth - s.marginLeft - s.marginRight;
    const [a, b] = ordered(this.selection);
    let r = deleteRange(this.blocks, a, b);
    if (r.pos.offset > 0) r = splitParagraph(r.blocks, r.pos);
    const blocks = r.blocks.slice();
    blocks.splice(r.pos.block, 0, newTable(rows, cols, width));
    const cont: ContainerRef = { type: "cell", table: r.pos.block, row: 0, col: 0 };
    this.commitDoc("table", { ...this.doc, blocks }, collapsedAt({ block: 0, offset: 0 }), cont);
    this.filterLines();
  }

  private tableEdit(fn: (t: Table, row: number, col: number) => Table | null, cell?: (t: Table, row: number, col: number) => [number, number]) {
    const tc = this.table();
    if (!tc || !this.doc) return false;
    const { t, c } = tc;
    const nt = fn(t, c.row, c.col);
    const blocks = this.doc.blocks.slice();
    if (!nt) {
      blocks.splice(c.table, 1, ...(blocks.length === 1 ? [paragraph()] : []));
      this.container = null;
      this.commitDoc("table", { ...this.doc, blocks: blocks.length ? blocks : [paragraph()] }, collapsedAt({ block: Math.min(c.table, blocks.length - 1), offset: 0 }), null);
      this.filterLines();
      return true;
    }
    blocks[c.table] = nt;
    let [row, col] = cell ? cell(nt, c.row, c.col) : [c.row, c.col];
    row = Math.max(0, Math.min(row, nt.rows.length - 1));
    col = Math.max(0, Math.min(col, nt.rows[row]!.cells.length - 1));
    this.commitDoc("table", { ...this.doc, blocks }, collapsedAt({ block: 0, offset: 0 }), { type: "cell", table: c.table, row, col });
    this.filterLines();
    return true;
  }

  insertNote(kind: NoteKind) {
    if (!this.doc || this.container?.type === "note" || this.container?.type === "header") return;
    const id = Math.max(0, ...(this.doc.notes ?? []).filter((n) => n.kind === kind).map((n) => n.id)) + 1;
    const [a, b] = ordered(this.selection);
    let r = deleteRange(this.blocks, a, b);
    r = insertInline(r.blocks, r.pos, { type: "note", kind, id, xml: "" });
    const withRef = withContainerBlocks(this.doc, this.container, r.blocks);
    const notes = [...(withRef.notes ?? []), { kind, id, blocks: [paragraph("", { style: "FootnoteText" })] }];
    this.commitDoc("insert", { ...withRef, notes }, collapsedAt({ block: 0, offset: 0 }), { type: "note", kind, id });
    this.filterLines();
  }

  editHeader(kind: "header" | "footer") {
    if (!this.doc) return;
    let index = (this.doc.headers ?? []).findIndex((h) => h.kind === kind && h.which === "default");
    if (index < 0) {
      const headers = [...(this.doc.headers ?? []), { kind, which: "default", relId: "", blocks: [paragraph()] }];
      index = headers.length - 1;
      this.commitDoc("insert", { ...this.doc, headers }, collapsedAt({ block: 0, offset: 0 }), { type: "header", index });
      this.filterLines();
      return;
    }
    this.setContainer({ type: "header", index });
  }

  newComment(text: string) {
    if (!this.doc || isCollapsed(this.selection)) return null;
    const ids = (this.doc.comments ?? []).map((c) => Number(c.id)).filter((n) => !Number.isNaN(n));
    const id = String(ids.length ? Math.max(...ids) + 1 : 0);
    const [a, b] = ordered(this.selection);
    let blocks = insertInline(this.blocks, b, { type: "comment", id, edge: "end" }).blocks;
    blocks = insertInline(blocks, { block: b.block, offset: b.offset + 1 }, { type: "comment", id, edge: "reference" }).blocks;
    blocks = insertInline(blocks, a, { type: "comment", id, edge: "start" }).blocks;
    const doc = withContainerBlocks(this.doc, this.container, blocks);
    const initials = this.author.split(/\s+/).map((w) => w[0] ?? "").join("").toUpperCase();
    const comments = [...(doc.comments ?? []), { id, author: this.author, initials, date: new Date().toISOString().replace(/\.\d+Z$/, "Z"), blocks: [paragraph(text)] }];
    this.commitDoc("comment", { ...doc, comments }, { anchor: { block: a.block, offset: a.offset + 1 }, focus: { block: b.block, offset: b.offset + (a.block === b.block ? 1 : 0) } });
    this.showSide(true);
    return id;
  }

  setCommentText(id: string, text: string) {
    if (!this.doc) return;
    const comments = (this.doc.comments ?? []).map((c) => (c.id === id ? { ...c, blocks: text.split("\n").map((l) => paragraph(l)) } : c));
    this.commitDoc("comment", { ...this.doc, comments });
  }

  deleteComment(id: string) {
    if (!this.doc) return;
    const doc = mapAllBlocks(this.doc, (bs) => bs.map((b) => (isParagraph(b) ? { ...b, inlines: b.inlines.filter((i) => !(i.type === "comment" && i.id === id)) } : b)));
    this.commitDoc("comment", { ...doc, comments: (doc.comments ?? []).filter((c) => c.id !== id) }, collapsedAt(clampPos(containerBlocks(doc, this.container), this.selection.focus)));
  }

  /** Select the text a comment is anchored to. */
  selectComment(id: string) {
    if (!this.doc) return;
    let start: Pos | null = null;
    let end: Pos | null = null;
    this.doc.blocks.forEach((b, bi) => {
      if (!isParagraph(b)) return;
      let at = 0;
      for (const i of b.inlines) {
        if (i.type === "comment" && i.id === id && i.edge === "start") start = { block: bi, offset: at + 1 };
        if (i.type === "comment" && i.id === id && i.edge === "end") end = { block: bi, offset: at };
        at += i.type === "text" ? i.text.length : 1;
      }
    });
    if (start && end) {
      this.container = null;
      this.filterLines();
      this.setSelection({ anchor: start, focus: end });
    }
  }

  /** Page number (1-based) of a body block, from the current layout. */
  pageOf = (block: number): number => {
    const hit = this.allLines.find((l) => !l.line.container && l.line.block === block);
    return (hit?.page ?? 0) + 1;
  };

  insertToc() {
    if (!this.doc) return;
    this.container = null;
    const at = Math.min(ordered(this.selection)[0].block, this.doc.blocks.length);
    const toc = buildToc(headings(this.doc), (b) => this.pageOf(b));
    const blocks = [...this.doc.blocks.slice(0, at), ...toc, ...this.doc.blocks.slice(at)];
    this.commitDoc("field", { ...this.doc, blocks }, collapsedAt({ block: at, offset: 0 }), null);
    // page numbers shift once the TOC itself takes space: refresh against the new layout
    this.doc = updateFields(this.doc, (b) => this.pageOf(b));
    this.relayout();
  }

  updateAllFields() {
    if (!this.doc) return;
    this.commitDoc("field", updateFields(this.doc, (b) => this.pageOf(b)));
  }

  insertCitation(tag: string) {
    if (!this.doc) return;
    const src = this.doc.sources?.find((s) => s.tag === tag);
    const [a, b] = ordered(this.selection);
    let r = deleteRange(this.blocks, a, b);
    for (const inl of citationField(src, tag)) r = insertInline(r.blocks, r.pos, inl);
    this.commit("field", r.blocks, collapsedAt(r.pos));
  }

  insertBibliography() {
    if (!this.doc) return;
    this.container = null;
    const at = Math.min(ordered(this.selection)[0].block + 1, this.doc.blocks.length);
    const bib = buildBibliography(this.doc.sources ?? []);
    this.commitDoc("field", { ...this.doc, blocks: [...this.doc.blocks.slice(0, at), ...bib, ...this.doc.blocks.slice(at)] }, collapsedAt({ block: at, offset: 0 }), null);
  }

  saveSource(source: Source) {
    if (!this.doc) return;
    const list = this.doc.sources ?? [];
    const sources = list.some((s) => s.tag === source.tag) ? list.map((s) => (s.tag === source.tag ? source : s)) : [...list, source];
    this.commitDoc("source", { ...this.doc, sources });
  }

  removeSource(tag: string) {
    if (!this.doc) return;
    this.commitDoc("source", { ...this.doc, sources: (this.doc.sources ?? []).filter((s) => s.tag !== tag) });
  }

  setTracking(on: boolean) {
    if (!this.doc) return;
    this.commitDoc("settings", { ...this.doc, settings: { trackRevisions: on, autoHyphenation: !!this.doc.settings?.autoHyphenation } });
  }

  resolveChanges(accept: boolean, all: boolean) {
    if (!this.doc) return;
    if (all) {
      this.commitDoc("revision", mapAllBlocks(this.doc, (bs) => resolveAll(bs, accept)));
      return;
    }
    const [a, b] = ordered(this.selection);
    this.commit("revision", resolveRange(this.blocks, a, b, accept), collapsedAt(a));
  }

  /** Plain text of the selection (paragraphs joined by newlines). */
  selectedText(): string {
    const [a, b] = ordered(this.selection);
    return blocksToText(sliceBlocks(this.blocks, a, b));
  }

  /** Replace the selection with plain text (keeps the formatting at its start). */
  replaceSelectionText(text: string) {
    const [a] = ordered(this.selection);
    const p = this.blocks[a.block];
    this.pendingMarks = p && isParagraph(p) ? (({ link: _l, revision: _r, ...m }) => m)(marksAt(p, a.offset + 1)) : null;
    const marks = this.typingMarks();
    const [, b] = ordered(this.selection);
    let { blocks, pos } = this.removeRange(a, b);
    if (this.tracking) pos = b;
    ({ blocks, pos } = insertText(blocks, pos, text, marks, this.nextStyleOf));
    this.commit("rewrite", blocks, collapsedAt(pos));
  }

  /** Insert AI-drafted content: blocks at the caret, plus its footnotes and sources. */
  insertDraft(draft: { blocks: Block[]; notes: { kind: NoteKind; id: number; blocks: Block[] }[]; sources: Source[] }) {
    if (!this.doc) return;
    this.container = null;
    this.filterLines();
    const at = Math.min(ordered(this.selection)[0].block + 1, this.doc.blocks.length);
    const cur = this.doc.blocks[at - 1];
    const replaceEmpty = cur && isParagraph(cur) && cur.inlines.length === 0;
    const blocks = [...this.doc.blocks.slice(0, replaceEmpty ? at - 1 : at), ...draft.blocks, ...this.doc.blocks.slice(at)];
    const sources = [...(this.doc.sources ?? [])];
    for (const s of draft.sources) if (!sources.some((x) => x.tag === s.tag)) sources.push(s);
    const end = (replaceEmpty ? at - 1 : at) + draft.blocks.length - 1;
    this.commitDoc("draft", { ...this.doc, blocks, notes: [...(this.doc.notes ?? []), ...draft.notes], sources }, collapsedAt({ block: Math.max(0, end), offset: Infinity }), null);
  }

  /** Next free footnote id. */
  nextNoteId(): number {
    return Math.max(0, ...(this.doc?.notes ?? []).filter((n) => n.kind === "footnote").map((n) => n.id)) + 1;
  }

  // ---- commands ------------------------------------------------------------------------

  execute(command: string, arg?: string): boolean {
    if (!this.doc) return false;
    switch (command) {
      case "bold":
      case "italic":
      case "underline":
      case "strike":
        this.toggleMark(command);
        break;
      case "align":
        this.updateParagraphs((p) => ({ ...p, align: (arg as Align) === "left" ? undefined : (arg as Align) }));
        break;
      case "alignCycle": {
        const order: (Align | undefined)[] = [undefined, "center", "right", "justify"];
        const cur = this.blocks[ordered(this.selection)[0].block];
        const now = cur && isParagraph(cur) ? cur.align : undefined;
        const next = order[(order.indexOf(now === "left" ? undefined : now) + 1) % order.length];
        this.updateParagraphs((p) => ({ ...p, align: next }));
        break;
      }
      case "rtl":
        this.updateParagraphs((p) => ({ ...p, bidi: !p.bidi || undefined }));
        break;
      case "bullets":
        this.toggleList(BULLET_NUM_ID);
        break;
      case "numbering":
        this.toggleList(DECIMAL_NUM_ID);
        break;
      case "indent":
        this.indent(1);
        break;
      case "outdent":
        this.indent(-1);
        break;
      case "style":
        this.updateParagraphs((p) => ({ ...p, style: !arg || arg === "Normal" ? undefined : arg }));
        break;
      case "font":
        this.setMarks({ font: arg || undefined });
        break;
      case "size":
        this.setMarks({ size: arg ? Math.round(Number(arg) * 2) : undefined });
        break;
      case "color":
        this.setMarks({ color: arg?.replace("#", "") || undefined });
        break;
      case "highlight":
        this.setMarks({ highlight: arg || undefined });
        break;
      case "link":
        this.setMarks({ link: arg || undefined });
        break;
      case "pageBreak":
        this.insertInlineAtCaret({ type: "break", kind: "page" });
        break;
      case "orientation": {
        const sec = this.doc.section;
        const landscape = arg ? arg === "landscape" : !sec.landscape;
        if (landscape !== sec.landscape) this.commitDoc("section", { ...this.doc, section: { ...sec, landscape, pageWidth: sec.pageHeight, pageHeight: sec.pageWidth } });
        break;
      }
      case "margins": {
        const presets: Record<string, [number, number]> = { normal: [1440, 1440], narrow: [720, 720], moderate: [1440, 1080], wide: [1440, 2880] };
        const m = presets[arg ?? "normal"];
        if (m) this.commitDoc("section", { ...this.doc, section: { ...this.doc.section, marginTop: m[0], marginBottom: m[0], marginLeft: m[1], marginRight: m[1] } });
        break;
      }
      case "columns": {
        const n = Math.max(1, Math.min(4, Number(arg) || 1));
        this.commitDoc("section", { ...this.doc, section: { ...this.doc.section, columns: n > 1 ? { count: n, space: 720, separator: false } : null } });
        break;
      }
      case "hyphenation":
        this.commitDoc("settings", { ...this.doc, settings: { trackRevisions: !!this.doc.settings?.trackRevisions, autoHyphenation: !this.doc.settings?.autoHyphenation } });
        break;
      case "insertTable": {
        const [r, c] = (arg ?? "3x3").split("x").map(Number);
        this.insertTable(r || 3, c || 3);
        break;
      }
      case "rowAbove":
        this.tableEdit((t, r) => tables.insertRow(t, r, false), (_t, r, c) => [r, c]);
        break;
      case "rowBelow":
        this.tableEdit((t, r) => tables.insertRow(t, r, true), (_t, r, c) => [r + 1, c]);
        break;
      case "colLeft":
        this.tableEdit((t, _r, c) => tables.insertColumn(t, c, false));
        break;
      case "colRight":
        this.tableEdit((t, _r, c) => tables.insertColumn(t, c, true), (_t, r, c) => [r, c + 1]);
        break;
      case "deleteRow":
        this.tableEdit((t, r) => tables.deleteRow(t, r));
        break;
      case "deleteColumn":
        this.tableEdit((t, _r, c) => tables.deleteColumn(t, c));
        break;
      case "deleteTable":
        this.tableEdit(() => null);
        break;
      case "mergeRight":
        this.tableEdit((t, r, c) => tables.mergeRight(t, r, c));
        break;
      case "splitCell":
        this.tableEdit((t, r, c) => tables.splitCell(t, r, c));
        break;
      case "headerRow":
        this.tableEdit((t) => tables.toggleHeaderRow(t));
        break;
      case "tableBorders":
        this.tableEdit((t) => ({ ...t, borders: !t.borders, props: undefined }));
        break;
      case "cellShading":
        this.tableEdit((t, r, c) => tables.setCell(t, r, c, { shading: arg ? arg.replace("#", "").toUpperCase() : undefined }));
        break;
      case "insertImage":
        void this.insertImage(arg);
        break;
      case "imageScale": {
        const f = Number(arg) || 1;
        this.replaceObject((i) => ({ ...i, cx: Math.round(i.cx * f), cy: Math.round(i.cy * f) }));
        break;
      }
      case "altText":
        this.replaceObject((i) => (i.type === "image" ? { ...i, descr: arg ?? "" } : i));
        break;
      case "insertShape":
        if (arg) this.insertInlineAtCaret(JSON.parse(arg) as Inline);
        break;
      case "footnote":
        this.insertNote("footnote");
        break;
      case "endnote":
        this.insertNote("endnote");
        break;
      case "header":
        this.editHeader("header");
        break;
      case "footer":
        this.editHeader("footer");
        break;
      case "exitContainer":
        this.exitContainer(1);
        break;
      case "trackChanges":
        this.setTracking(!this.tracking);
        break;
      case "acceptAll":
        this.resolveChanges(true, true);
        break;
      case "rejectAll":
        this.resolveChanges(false, true);
        break;
      case "accept":
        this.resolveChanges(true, false);
        break;
      case "reject":
        this.resolveChanges(false, false);
        break;
      case "nextChange": {
        const p = nextRevision(this.blocks, this.selection.focus);
        if (p) this.setSelection(collapsedAt(p));
        break;
      }
      case "markup":
        this.showMarkup = !this.showMarkup;
        this.relayout();
        break;
      case "comment":
        if (!this.newComment(arg ?? "")) return false;
        break;
      case "comments":
        this.showSide(this.side.hidden);
        break;
      case "toc":
        this.insertToc();
        break;
      case "updateFields":
        this.updateAllFields();
        break;
      case "citation":
        if (arg) this.insertCitation(arg);
        break;
      case "bibliography":
        this.insertBibliography();
        break;
      case "undo":
        this.undo();
        break;
      case "redo":
        this.redo();
        break;
      case "selectAll":
        this.selectAll();
        break;
      case "markdown":
        this.toggleMarkdown();
        break;
      default:
        return false;
    }
    this.scheduleRender();
    this.focus();
    return true;
  }

  /** Formatting state at the selection (for ribbon toggles and pickers). */
  state() {
    const [a, b] = ordered(this.selection);
    const p = this.blocks[a.block];
    const para = p && isParagraph(p) ? p : null;
    const collapsed = isCollapsed(this.selection);
    const marks = collapsed ? (this.pendingMarks ?? (para ? marksAt(para, a.offset) : {})) : null;
    const has = (m: "bold" | "italic" | "underline" | "strike") => (collapsed ? !!marks![m] : rangeHasMark(this.blocks, a, b, m));
    return {
      bold: has("bold"),
      italic: has("italic"),
      underline: has("underline"),
      strike: has("strike"),
      style: para?.style ?? "Normal",
      align: para?.align ?? "left",
      list: para?.list?.numId ?? null,
      font: (para ? marksAt(para, a.offset).font : undefined) ?? this.doc?.defaultRun.font ?? "Calibri",
      size: ((para ? marksAt(para, a.offset).size : undefined) ?? this.doc?.defaultRun.size ?? 22) / 2,
      container: this.container?.type ?? "body",
      inTable: this.container?.type === "cell",
      tracking: this.tracking,
      showMarkup: this.showMarkup,
      object: this.selectedObject()?.inline ?? null,
      rtl: !!para?.bidi,
      hyphenation: !!this.doc?.settings?.autoHyphenation,
      columns: this.doc?.section.columns?.count ?? 1,
    };
  }

  // ---- side panel (comments) ----------------------------------------------------------------

  showSide(on: boolean) {
    this.side.hidden = !on;
    this.el.classList.toggle("with-side", on);
    this.refreshSide();
    requestAnimationFrame(() => this.render());
  }

  private refreshSide() {
    if (this.side.hidden || !this.doc) return;
    const comments = this.doc.comments ?? [];
    const text = (bs: Block[]) => bs.map((b) => (isParagraph(b) ? b.inlines.map((i) => (i.type === "text" ? i.text : "")).join("") : "")).join("\n");
    this.side.replaceChildren(
      h("h3", { class: "side-title" }, `Comments (${comments.length})`),
      ...(comments.length
        ? comments.map((c) =>
            h(
              "article",
              { class: "comment-card", dataset: { id: c.id } },
              h(
                "header",
                { class: "comment-head", onclick: () => this.selectComment(c.id) },
                h("span", { class: "comment-avatar" }, c.initials || "?"),
                h("strong", {}, c.author || "Unknown"),
                h("time", { class: "muted small" }, c.date ? new Date(c.date).toLocaleDateString() : ""),
              ),
              h("textarea", {
                class: "comment-text",
                rows: "3",
                "aria-label": `Comment by ${c.author}`,
                onchange: (e: Event) => this.setCommentText(c.id, (e.target as HTMLTextAreaElement).value),
              }, text(c.blocks)),
              h("button", { class: "link comment-delete", onclick: () => this.deleteComment(c.id) }, "Delete"),
            ),
          )
        : [h("p", { class: "muted small" }, "Select text and add a comment.")]),
    );
  }

  // ---- markdown split view ------------------------------------------------------------------

  get markdownVisible() {
    return !this.mdPane.hidden;
  }

  toggleMarkdown() {
    this.mdPane.hidden = !this.mdPane.hidden;
    this.el.classList.toggle("split", !this.mdPane.hidden);
    if (!this.mdPane.hidden) this.refreshMarkdown();
    requestAnimationFrame(() => this.render());
  }

  private refreshMarkdown() {
    const { markdown, objects } = toMarkdown(this.doc?.blocks ?? []);
    this.mdObjects = objects;
    if (this.mdPane.value !== markdown) this.mdPane.value = markdown;
  }

  /** Re-import the Markdown pane into the document body. */
  applyMarkdown(md = this.mdPane.value) {
    if (!this.doc) return;
    const blocks = fromMarkdown(md, this.mdObjects);
    const sel = { anchor: clampPos(blocks, this.selection.anchor), focus: clampPos(blocks, this.selection.focus) };
    this.container = null;
    this.filterLines();
    this.commitDoc("markdown", { ...this.doc, blocks }, sel, null);
  }

  // ---- events -------------------------------------------------------------------------------

  private bind() {
    const contentPoint = (e: MouseEvent) => {
      const r = this.scroller.getBoundingClientRect();
      return { x: e.clientX - r.left + this.scroller.scrollLeft, y: e.clientY - r.top + this.scroller.scrollTop };
    };
    this.scroller.addEventListener("scroll", () => this.render());
    this.scroller.addEventListener("mousedown", (e) => {
      if (e.button !== 0) return;
      e.preventDefault();
      this.focus();
      const pt = contentPoint(e);
      if (!e.shiftKey) {
        const target = this.hitTarget(pt.x, pt.y, e.detail >= 2);
        if (!sameContainer(target.container, this.container)) {
          this.container = target.container;
          this.filterLines();
          this.refreshSide();
        }
      }
      const p = this.hitTest(pt.x, pt.y);
      if (e.detail === 2) {
        this.setSelection({ anchor: wordBoundary(this.blocks, { ...p, offset: p.offset + 1 }, -1), focus: wordBoundary(this.blocks, p, 1) });
        return;
      }
      if (e.detail >= 3) {
        this.setSelection({ anchor: { block: p.block, offset: 0 }, focus: { block: p.block, offset: blockLength(this.blocks[p.block]!) } });
        return;
      }
      this.dragging = true;
      this.setSelection(e.shiftKey ? { anchor: this.selection.anchor, focus: p } : collapsedAt(p));
    });
    const onMove = (e: MouseEvent) => {
      if (!this.dragging) return;
      const pt = contentPoint(e);
      this.setSelection({ anchor: this.selection.anchor, focus: this.hitTest(pt.x, pt.y) });
    };
    const onUp = () => (this.dragging = false);
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);

    this.input.addEventListener("keydown", (e) => this.onKeyDown(e));
    this.input.addEventListener("compositionstart", () => (this.composing = true));
    this.input.addEventListener("compositionend", (e) => {
      this.composing = false;
      if (e.data) this.insertText(e.data);
      this.input.value = "";
    });
    this.input.addEventListener("input", () => {
      if (this.composing) return;
      const v = this.input.value;
      this.input.value = "";
      if (v) this.insertText(v);
    });
    this.input.addEventListener("focus", () => this.startBlink());
    this.input.addEventListener("blur", () => this.scheduleRender());
    this.input.addEventListener("copy", (e) => this.onCopy(e, false));
    this.input.addEventListener("cut", (e) => this.onCopy(e, true));
    this.input.addEventListener("paste", (e) => void this.onPaste(e));

    this.mdPane.addEventListener("input", () => {
      if (this.mdTimer) clearTimeout(this.mdTimer);
      this.mdTimer = setTimeout(() => this.applyMarkdown(), 300);
    });

    if (typeof ResizeObserver !== "undefined") {
      this.resizeObs = new ResizeObserver(() => this.render());
      this.resizeObs.observe(this.scroller);
    }
  }

  private startBlink() {
    if (this.blink) clearInterval(this.blink);
    this.caretOn = true;
    this.scheduleRender();
    this.blink = setInterval(() => {
      if (document.activeElement !== this.input) return;
      this.caretOn = !this.caretOn;
      this.render();
    }, 530);
  }

  private onCopy(e: ClipboardEvent, cut: boolean) {
    if (isCollapsed(this.selection)) return;
    e.preventDefault();
    const [a, b] = ordered(this.selection);
    const frag = sliceBlocks(this.blocks, a, b);
    e.clipboardData?.setData("text/plain", blocksToText(frag));
    e.clipboardData?.setData("text/html", blocksToHtml(frag));
    e.clipboardData?.setData("text/rtf", blocksToRtf(frag));
    e.clipboardData?.setData("application/x-prism-blocks", JSON.stringify(frag));
    if (cut) this.deleteSelectionOr(1);
  }

  private async onPaste(e: ClipboardEvent) {
    e.preventDefault();
    const dt = e.clipboardData;
    if (!dt) return;
    const native = dt.getData("application/x-prism-blocks");
    if (native) {
      try {
        this.insertBlocksAtSelection(JSON.parse(native) as Block[]);
        return;
      } catch {
        /* fall through */
      }
    }
    const file = [...(dt.files ?? [])].find((f) => f.type.startsWith("image/"));
    if (file) {
      const data = await new Promise<string | null>((resolve) => {
        const r = new FileReader();
        r.onload = () => resolve(String(r.result).split(",")[1] ?? null);
        r.onerror = () => resolve(null);
        r.readAsDataURL(file);
      });
      if (data) await this.insertImage(data);
      return;
    }
    const html = dt.getData("text/html");
    if (html) {
      const blocks = htmlToBlocks(html);
      if (blocks.length) {
        this.insertBlocksAtSelection(this.container ? blocks.filter((b) => !isTable(b)) : blocks);
        return;
      }
    }
    const rtf = dt.getData("text/rtf");
    if (rtf) {
      const blocks = rtfToBlocks(rtf);
      if (blocks.length) return this.insertBlocksAtSelection(blocks);
    }
    const text = dt.getData("text/plain");
    if (text) this.insertBlocksAtSelection(textToBlocks(text));
  }

  private onKeyDown(e: KeyboardEvent) {
    if (this.composing || e.isComposing) return;
    const mod = e.metaKey || e.ctrlKey;
    const word = e.altKey || (e.ctrlKey && !e.metaKey);
    const sel = this.selection;
    const move = (p: Pos, keepGoal = false) => {
      e.preventDefault();
      // entering a table from the body
      const tb = !this.container && !e.shiftKey ? this.blocks[p.block] : undefined;
      if (tb && isTable(tb)) return this.enterTable(p.block, comparePos(p, sel.focus) < 0);
      this.setSelection(e.shiftKey ? { anchor: sel.anchor, focus: p } : collapsedAt(p), keepGoal);
    };
    const collapseTo = (dir: 1 | -1) => (isCollapsed(sel) || e.shiftKey ? null : ordered(sel)[dir > 0 ? 1 : 0]);
    const inCell = this.container?.type === "cell";
    const atStart = sel.focus.block === 0 && sel.focus.offset === 0;
    const atEnd = sel.focus.block === this.blocks.length - 1 && sel.focus.offset === blockLength(this.blocks[sel.focus.block]!);
    switch (e.key) {
      case "Escape":
        if (this.container) {
          e.preventDefault();
          this.exitContainer(1);
        }
        return;
      case "ArrowLeft":
        if (inCell && atStart && isCollapsed(sel) && !e.shiftKey) {
          e.preventDefault();
          return void this.moveCell(-1, false);
        }
        return move(collapseTo(-1) ?? (word ? wordBoundary(this.blocks, sel.focus, -1) : e.metaKey ? this.lineEdge(sel.focus, false) : stepPos(this.blocks, sel.focus, -1)));
      case "ArrowRight":
        if (inCell && atEnd && isCollapsed(sel) && !e.shiftKey) {
          e.preventDefault();
          return void this.moveCell(1, false);
        }
        return move(collapseTo(1) ?? (word ? wordBoundary(this.blocks, sel.focus, 1) : e.metaKey ? this.lineEdge(sel.focus, true) : stepPos(this.blocks, sel.focus, 1)));
      case "ArrowUp":
        if (inCell && this.isFirstLine(sel.focus) && !e.shiftKey) {
          e.preventDefault();
          return void this.moveCell(-1, true);
        }
        return move(e.metaKey ? { block: 0, offset: 0 } : this.moveVertical(sel.focus, -1), !e.metaKey);
      case "ArrowDown":
        if (inCell && this.isLastLine(sel.focus) && !e.shiftKey) {
          e.preventDefault();
          return void this.moveCell(1, true);
        }
        return move(e.metaKey ? clampPos(this.blocks, { block: this.blocks.length - 1, offset: Infinity }) : this.moveVertical(sel.focus, 1), !e.metaKey);
      case "PageUp":
      case "PageDown": {
        const n = Math.max(1, Math.floor(this.scroller.clientHeight / 22));
        return move(this.moveVertical(sel.focus, e.key === "PageUp" ? -1 : 1, n), true);
      }
      case "Home":
        return move(mod ? { block: 0, offset: 0 } : this.lineEdge(sel.focus, false));
      case "End":
        return move(mod ? clampPos(this.blocks, { block: this.blocks.length - 1, offset: Infinity }) : this.lineEdge(sel.focus, true));
      case "Backspace":
        e.preventDefault();
        return this.deleteSelectionOr(-1, word);
      case "Delete":
        e.preventDefault();
        return this.deleteSelectionOr(1, word);
      case "Enter": {
        e.preventDefault();
        if (mod) return void this.execute("pageBreak");
        if (e.shiftKey) return this.insertInlineAtCaret({ type: "break", kind: "line" }, "type");
        const blk = this.blocks[sel.focus.block];
        if (isCollapsed(sel) && blk && isParagraph(blk) && blk.list && blockLength(blk) === 0) {
          return this.commit("format", updateParagraphs(this.blocks, sel.focus, sel.focus, (p) => ({ ...p, list: undefined })), sel);
        }
        const [a, b] = ordered(sel);
        let r = deleteRange(this.blocks, a, b);
        r = splitParagraph(r.blocks, r.pos, this.nextStyleOf);
        return this.commit("split", r.blocks, collapsedAt(r.pos));
      }
      case "Tab": {
        e.preventDefault();
        if (inCell) return void this.moveCell(e.shiftKey ? -1 : 1, false, !e.shiftKey);
        const blk = this.blocks[sel.focus.block];
        if (blk && isParagraph(blk) && blk.list) return this.indent(e.shiftKey ? -1 : 1);
        if (e.shiftKey) return this.indent(-1);
        return this.insertInlineAtCaret({ type: "tab", marks: this.typingMarks() }, "type");
      }
    }
    if (!mod || e.altKey) return;
    const k = e.key.toLowerCase();
    const map: Record<string, () => void> = {
      b: () => this.toggleMark("bold"),
      i: () => this.toggleMark("italic"),
      u: () => this.toggleMark("underline"),
      a: () => this.selectAll(),
      z: () => (e.shiftKey ? this.redo() : this.undo()),
      y: () => this.redo(),
      e: () => this.execute("align", "center"),
      l: () => this.execute("align", "left"),
      r: () => this.execute("align", "right"),
      j: () => this.execute("align", "justify"),
    };
    const fn = map[k];
    if (fn) {
      e.preventDefault();
      e.stopPropagation();
      fn();
      this.scheduleRender();
    }
  }
}
