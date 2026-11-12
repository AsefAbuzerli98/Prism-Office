/**
 * WYSIWYG document editor: canvas rendering, caret/selection, keyboard,
 * IME, clipboard, undo/redo and a split-pane Markdown source view.
 */
import { h } from "../core/dom";
import type { HostBridge } from "../core/host";
import { blocksToHtml, blocksToText, htmlToBlocks, textToBlocks } from "./clipboard";
import { History } from "./history";
import { canvasMeasurer, layoutDocument, type Layout, type Line, type Measurer } from "./layout";
import { fromMarkdown, setBulletIds, toMarkdown } from "./markdown";
import { BULLET_NUM_ID, DECIMAL_NUM_ID, isParagraph, type Align, type Block, type Inline, type Marks, type Paragraph, type WordDocument } from "./model";
import {
  applyMarks, blockLength, clampPos, comparePos, deleteRange, insertBlocks, insertInline, insertText, isCollapsed, marksAt,
  ordered, rangeHasMark, sliceBlocks, splitParagraph, stepPos, updateParagraphs, wordBoundary, type Pos, type Selection,
} from "./ops";
import { pageOrigin, paint, totalHeight, type Rect } from "./render";

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

export interface EditorOptions {
  onDirty?: () => void;
  /** Selection or formatting state changed (ribbon refresh). */
  onState?: () => void;
  onChange?: (doc: WordDocument) => void;
  measurer?: Measurer;
  syncDelayMs?: number;
}

export class WordEditor implements EngineView {
  readonly el: HTMLElement;
  doc: WordDocument | null = null;
  selection: Selection = { anchor: { block: 0, offset: 0 }, focus: { block: 0, offset: 0 } };
  readonly history = new History();
  private layout: Layout | null = null;
  private lines: Placed[] = [];
  private readonly scroller: HTMLElement;
  private readonly spacer: HTMLElement;
  private readonly canvas: HTMLCanvasElement;
  private readonly input: HTMLTextAreaElement;
  private readonly mdPane: HTMLTextAreaElement;
  private readonly measurer: Measurer;
  private pendingMarks: Marks | null = null;
  private goalX: number | null = null;
  private composing = false;
  private dragging = false;
  private caretOn = true;
  private blink: ReturnType<typeof setInterval> | null = null;
  private syncTimer: ReturnType<typeof setTimeout> | null = null;
  private syncing: Promise<void> = Promise.resolve();
  private mdTimer: ReturnType<typeof setTimeout> | null = null;
  private mdObjects = new Map<string, Block | Inline>();
  private dirty = false;
  private raf = 0;
  private resizeObs: ResizeObserver | null = null;
  readonly ready: Promise<void>;

  constructor(
    private readonly host: HostBridge,
    readonly docId: number,
    private readonly opts: EditorOptions = {},
  ) {
    this.measurer = opts.measurer ?? canvasMeasurer();
    this.canvas = h("canvas", { class: "word-canvas", "aria-hidden": "true" });
    this.spacer = h("div", { class: "word-spacer" });
    this.input = h("textarea", {
      class: "word-input",
      "aria-label": "Document text",
      autocapitalize: "off",
      autocomplete: "off",
      spellcheck: "false",
    });
    this.scroller = h("div", { class: "word-scroll", tabindex: "-1" }, this.canvas, this.spacer, this.input);
    this.mdPane = h("textarea", { class: "word-md", "aria-label": "Markdown source", spellcheck: "false", hidden: true });
    this.el = h("div", { class: "word-editor" }, this.scroller, this.mdPane);
    this.bind();
    this.ready = this.load();
  }

  // ---- lifecycle -------------------------------------------------------------

  private async load() {
    const doc = await this.host.getWordDocument(this.docId);
    this.doc = doc;
    setBulletIds(doc.numbering.filter((n) => n.levels[0]?.format === "bullet").map((n) => n.numId));
    this.selection = { anchor: { block: 0, offset: 0 }, focus: { block: 0, offset: 0 } };
    this.relayout();
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

  private get blocks(): Block[] {
    return this.doc?.blocks ?? [];
  }

  /** Apply a new block array as one undoable step. */
  commit(kind: string, blocks: Block[], selection: Selection) {
    if (!this.doc) return;
    this.history.record({ blocks: this.doc.blocks, selection: this.selection }, kind);
    this.doc = { ...this.doc, blocks };
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
    this.opts.onChange?.(this.doc!);
    this.opts.onState?.();
  }

  /** Mark saved: next edit fires onDirty again. */
  markClean() {
    this.dirty = false;
  }

  private setSelection(sel: Selection, keepGoal = false) {
    this.selection = sel;
    if (!keepGoal) this.goalX = null;
    this.pendingMarks = null;
    this.history.seal();
    this.caretOn = true;
    this.scheduleRender();
    this.scrollCaretIntoView();
    this.opts.onState?.();
  }

  // ---- layout & paint ----------------------------------------------------------

  private relayout() {
    if (!this.doc) return;
    this.layout = layoutDocument(this.doc, this.measurer);
    this.lines = this.layout.pages.flatMap((p) => p.lines.map((line) => ({ line, page: p.index })));
    this.spacer.style.height = `${totalHeight(this.layout)}px`;
    this.spacer.style.width = `${this.layout.width + 48}px`;
    this.scheduleRender();
  }

  private scheduleRender() {
    if (this.raf) return;
    this.raf = requestAnimationFrame(() => {
      this.raf = 0;
      this.render();
    });
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
    paint(ctx, this.layout, vp, dpr, this.selectionRects(), caret, document.documentElement.dataset.appearance === "dark");
    this.positionInput();
  }

  private viewport() {
    return {
      scrollTop: this.scroller.scrollTop,
      scrollLeft: this.scroller.scrollLeft,
      width: this.scroller.clientWidth,
      height: this.scroller.clientHeight,
    };
  }

  // ---- geometry ------------------------------------------------------------------

  /** Line index containing a position (end-of-line belongs to the line). */
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
    const vpw = this.scroller.clientWidth || this.layout.width + 48;
    const idx = this.lineIndexOf(p);
    if (idx < 0) {
      // opaque block
      for (const page of this.layout.pages) {
        const box = page.boxes.find((b) => b.block === p.block);
        if (box) {
          const o = pageOrigin(this.layout, page.index, vpw);
          return { x: o.x + (p.offset ? box.right : box.left) - 2, y: o.y + box.y, w: 2, h: box.height };
        }
      }
      return null;
    }
    const { line, page } = this.lines[idx]!;
    const o = pageOrigin(this.layout, page, vpw);
    return { x: o.x + this.xInLine(line, p.offset), y: o.y + line.y, w: 2, h: line.height };
  }

  private xInLine(line: Line, offset: number): number {
    const frags = line.frags.filter((f) => f.kind !== "marker");
    for (const f of frags) {
      if (offset >= f.start && offset <= f.end && (offset < f.end || f === frags[frags.length - 1] || f.kind !== "text")) {
        if (f.kind !== "text") return offset === f.start ? f.x : f.x + f.width;
        return f.x + this.measurer.width(f.text.slice(0, offset - f.start), f.style.font);
      }
    }
    const last = frags[frags.length - 1];
    return last ? last.x + last.width : line.left;
  }

  /** Map a point in scroller content coordinates to a position. */
  hitTest(x: number, y: number): Pos {
    if (!this.layout || this.lines.length === 0) return { block: 0, offset: 0 };
    const vpw = this.scroller.clientWidth || this.layout.width + 48;
    const pageIdx = Math.max(0, Math.min(this.layout.pages.length - 1, Math.floor((y - 12) / (this.layout.height + 24))));
    const o = pageOrigin(this.layout, pageIdx, vpw);
    const px = x - o.x;
    const py = y - o.y;
    const page = this.layout.pages[pageIdx]!;
    for (const b of page.boxes) {
      if (py >= b.y && py <= b.y + b.height) return { block: b.block, offset: px > (b.left + b.right) / 2 ? 1 : 0 };
    }
    let best: Line | null = null;
    let bestD = Infinity;
    for (const line of page.lines) {
      const d = py < line.y ? line.y - py : py > line.y + line.height ? py - line.y - line.height : 0;
      if (d < bestD) {
        bestD = d;
        best = line;
      }
    }
    if (!best) {
      const lastLine = this.lines[this.lines.length - 1]!.line;
      return { block: lastLine.block, offset: lastLine.end };
    }
    return { block: best.block, offset: this.offsetInLine(best, px) };
  }

  private offsetInLine(line: Line, px: number): number {
    const frags = line.frags.filter((f) => f.kind !== "marker");
    for (const f of frags) {
      if (px < f.x + f.width) {
        if (px <= f.x) return f.start;
        if (f.kind !== "text") return px > f.x + f.width / 2 ? f.end : f.start;
        let prev = f.x;
        for (let i = 1; i <= f.text.length; i++) {
          const cx = f.x + this.measurer.width(f.text.slice(0, i), f.style.font);
          if (px < (prev + cx) / 2) return f.start + i - 1;
          prev = cx;
        }
        return f.end;
      }
    }
    // Past the end: before a trailing space on wrapped lines.
    const last = frags[frags.length - 1];
    if (!line.last && last?.kind === "text" && /\s$/.test(last.text)) return line.end - 1;
    return line.end;
  }

  private selectionRects(): Rect[] {
    if (!this.layout || isCollapsed(this.selection)) return [];
    const [a, b] = ordered(this.selection);
    const vpw = this.scroller.clientWidth || this.layout.width + 48;
    const rects: Rect[] = [];
    for (const { line, page } of this.lines) {
      if (line.block < a.block || line.block > b.block) continue;
      const s = line.block === a.block ? Math.max(a.offset, line.start) : line.start;
      const e = line.block === b.block ? Math.min(b.offset, line.end) : line.end;
      if (s > e || (s === e && !(line.block < b.block && line.last))) continue;
      const o = pageOrigin(this.layout, page, vpw);
      const x1 = this.xInLine(line, s);
      let x2 = this.xInLine(line, e);
      if (line.last && line.block < b.block) x2 += 6; // show paragraph mark selected
      rects.push({ x: o.x + x1, y: o.y + line.y, w: Math.max(2, x2 - x1), h: line.height });
    }
    for (const page of this.layout.pages) {
      for (const box of page.boxes) {
        if (comparePos({ block: box.block, offset: 0 }, a) >= 0 && comparePos({ block: box.block, offset: 1 }, b) <= 0) {
          const o = pageOrigin(this.layout, page.index, vpw);
          rects.push({ x: o.x + box.left, y: o.y + box.y, w: box.right - box.left, h: box.height });
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

  // ---- vertical movement -----------------------------------------------------------

  private moveVertical(p: Pos, dir: 1 | -1, lines = 1): Pos {
    const cur = this.caretRect(p);
    if (!cur || !this.layout) return p;
    const x = this.goalX ?? cur.x;
    this.goalX = x;
    let idx = this.lineIndexOf(p);
    if (idx < 0) {
      // from an opaque block: nearest line before/after
      idx = dir > 0 ? this.lines.findIndex((l) => l.line.block > p.block) : this.lines.map((l) => l.line.block < p.block).lastIndexOf(true);
      if (idx < 0) return dir > 0 ? clampPos(this.blocks, { block: this.blocks.length - 1, offset: Infinity }) : { block: 0, offset: 0 };
      lines -= 1;
    }
    const target = Math.max(0, Math.min(this.lines.length - 1, idx + dir * lines));
    // Opaque blocks between lines are stops of their own.
    const between = this.blocks.findIndex((b, bi) => !isParagraph(b) && (dir > 0 ? bi > p.block && bi < this.lines[target]!.line.block : bi < p.block && bi > this.lines[target]!.line.block));
    if (lines === 1 && between >= 0) return { block: between, offset: 0 };
    if (target === idx) return dir > 0 ? { block: p.block, offset: this.lines[idx]!.line.end } : { block: p.block, offset: this.lines[idx]!.line.start };
    const { line, page } = this.lines[target]!;
    const o = pageOrigin(this.layout, page, this.scroller.clientWidth || this.layout.width + 48);
    return { block: line.block, offset: this.offsetInLine(line, x - o.x) };
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

  // ---- editing primitives -------------------------------------------------------------

  private nextStyleOf = (style: string | undefined) => {
    const s = this.doc?.styles.find((x) => x.id === style);
    return s?.next;
  };

  private typingMarks(): Marks {
    if (this.pendingMarks) return this.pendingMarks;
    const p = this.blocks[this.selection.focus.block];
    if (!p || !isParagraph(p)) return {};
    const { link: _link, ...rest } = marksAt(p, this.selection.focus.offset);
    return rest;
  }

  /** Replace the selection with text. */
  insertText(text: string, kind = "type") {
    if (!this.doc) return;
    const marks = this.typingMarks();
    const [a, b] = ordered(this.selection);
    let { blocks, pos } = deleteRange(this.blocks, a, b);
    ({ blocks, pos } = insertText(blocks, pos, text, marks, this.nextStyleOf));
    this.commit(kind, blocks, { anchor: pos, focus: pos });
    this.pendingMarks = null;
  }

  private insertBlocksAtSelection(frag: Block[]) {
    const [a, b] = ordered(this.selection);
    let { blocks, pos } = deleteRange(this.blocks, a, b);
    ({ blocks, pos } = insertBlocks(blocks, pos, frag));
    this.commit("paste", blocks, { anchor: pos, focus: pos });
  }

  deleteSelectionOr(dir: 1 | -1, word = false) {
    if (!isCollapsed(this.selection)) {
      const [a, b] = ordered(this.selection);
      const r = deleteRange(this.blocks, a, b);
      this.commit("delete", r.blocks, { anchor: r.pos, focus: r.pos });
      return;
    }
    const p = this.selection.focus;
    // Backspace at the start of a list item first removes the list formatting.
    const blk = this.blocks[p.block];
    if (dir < 0 && p.offset === 0 && blk && isParagraph(blk) && (blk.list || (blk.style && blk.style !== "Normal" && blockLength(blk) === 0))) {
      const next = updateParagraphs(this.blocks, p, p, (q) => (q.list ? { ...q, list: undefined } : { ...q, style: undefined }));
      this.commit("format", next, this.selection);
      return;
    }
    const q = word ? wordBoundary(this.blocks, p, dir) : stepPos(this.blocks, p, dir);
    if (comparePos(p, q) === 0) return;
    const r = deleteRange(this.blocks, p, q);
    this.commit("delete", r.blocks, { anchor: r.pos, focus: r.pos });
  }

  toggleMark(mark: "bold" | "italic" | "underline" | "strike") {
    if (isCollapsed(this.selection)) {
      const m = { ...this.typingMarks() };
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
      this.pendingMarks = { ...this.typingMarks(), ...patch };
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
      const cur = p.indentLeft ?? 0;
      const v = Math.max(0, cur + dir * 720);
      return { ...p, indentLeft: v || undefined };
    });
  }

  undo() {
    if (!this.doc) return;
    const s = this.history.undo({ blocks: this.doc.blocks, selection: this.selection });
    if (s) this.restore(s.blocks, s.selection);
  }

  redo() {
    if (!this.doc) return;
    const s = this.history.redo({ blocks: this.doc.blocks, selection: this.selection });
    if (s) this.restore(s.blocks, s.selection);
  }

  private restore(blocks: Block[], selection: Selection) {
    this.doc = { ...this.doc!, blocks };
    this.selection = selection;
    this.changed();
  }

  selectAll() {
    const last = this.blocks.length - 1;
    this.setSelection({ anchor: { block: 0, offset: 0 }, focus: { block: last, offset: blockLength(this.blocks[last]!) } });
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
      case "link":
        this.setMarks({ link: arg || undefined });
        break;
      case "pageBreak": {
        const r = insertInline(this.blocks, ordered(this.selection)[0], { type: "break", kind: "page" });
        this.commit("insert", r.blocks, { anchor: r.pos, focus: r.pos });
        break;
      }
      case "orientation": {
        const sec = this.doc.section;
        const landscape = arg ? arg === "landscape" : !sec.landscape;
        if (landscape !== sec.landscape) {
          this.doc = { ...this.doc, section: { ...sec, landscape, pageWidth: sec.pageHeight, pageHeight: sec.pageWidth } };
          this.changed();
        }
        break;
      }
      case "margins": {
        const presets: Record<string, [number, number]> = { normal: [1440, 1440], narrow: [720, 720], moderate: [1440, 1080], wide: [1440, 2880] };
        const m = presets[arg ?? "normal"];
        if (m) {
          this.doc = { ...this.doc, section: { ...this.doc.section, marginTop: m[0], marginBottom: m[0], marginLeft: m[1], marginRight: m[1] } };
          this.changed();
        }
        break;
      }
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
    const marks = collapsed ? this.typingMarks() : null;
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
    };
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
    const { markdown, objects } = toMarkdown(this.blocks);
    this.mdObjects = objects;
    if (this.mdPane.value !== markdown) this.mdPane.value = markdown;
  }

  /** Re-import the Markdown pane into the document. */
  applyMarkdown(md = this.mdPane.value) {
    const blocks = fromMarkdown(md, this.mdObjects);
    const sel = { anchor: clampPos(blocks, this.selection.anchor), focus: clampPos(blocks, this.selection.focus) };
    this.commit("markdown", blocks, sel);
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
      this.setSelection(e.shiftKey ? { anchor: this.selection.anchor, focus: p } : { anchor: p, focus: p });
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
    this.input.addEventListener("paste", (e) => this.onPaste(e));

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
    e.clipboardData?.setData("application/x-prism-blocks", JSON.stringify(frag));
    if (cut) this.deleteSelectionOr(1);
  }

  private onPaste(e: ClipboardEvent) {
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
    const html = dt.getData("text/html");
    if (html) {
      const blocks = htmlToBlocks(html);
      if (blocks.length) {
        this.insertBlocksAtSelection(blocks);
        return;
      }
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
      this.setSelection(e.shiftKey ? { anchor: sel.anchor, focus: p } : { anchor: p, focus: p }, keepGoal);
    };
    const collapseTo = (dir: 1 | -1) => (isCollapsed(sel) || e.shiftKey ? null : ordered(sel)[dir > 0 ? 1 : 0]);
    switch (e.key) {
      case "ArrowLeft":
        return move(collapseTo(-1) ?? (word ? wordBoundary(this.blocks, sel.focus, -1) : e.metaKey ? this.lineEdge(sel.focus, false) : stepPos(this.blocks, sel.focus, -1)));
      case "ArrowRight":
        return move(collapseTo(1) ?? (word ? wordBoundary(this.blocks, sel.focus, 1) : e.metaKey ? this.lineEdge(sel.focus, true) : stepPos(this.blocks, sel.focus, 1)));
      case "ArrowUp":
        return move(e.metaKey ? { block: 0, offset: 0 } : this.moveVertical(sel.focus, -1), !e.metaKey);
      case "ArrowDown":
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
        if (e.shiftKey) {
          const [a, b] = ordered(sel);
          let r = deleteRange(this.blocks, a, b);
          r = insertInline(r.blocks, r.pos, { type: "break", kind: "line" });
          return this.commit("type", r.blocks, { anchor: r.pos, focus: r.pos });
        }
        const blk = this.blocks[sel.focus.block];
        // Enter on an empty list item ends the list.
        if (isCollapsed(sel) && blk && isParagraph(blk) && blk.list && blockLength(blk) === 0) {
          return this.commit("format", updateParagraphs(this.blocks, sel.focus, sel.focus, (p) => ({ ...p, list: undefined })), sel);
        }
        const [a, b] = ordered(sel);
        let r = deleteRange(this.blocks, a, b);
        r = splitParagraph(r.blocks, r.pos, this.nextStyleOf);
        return this.commit("split", r.blocks, { anchor: r.pos, focus: r.pos });
      }
      case "Tab": {
        e.preventDefault();
        const blk = this.blocks[sel.focus.block];
        if (blk && isParagraph(blk) && blk.list) return this.indent(e.shiftKey ? -1 : 1);
        if (e.shiftKey) return this.indent(-1);
        const [a, b] = ordered(sel);
        let r = deleteRange(this.blocks, a, b);
        r = insertInline(r.blocks, r.pos, { type: "tab", marks: this.typingMarks() });
        return this.commit("type", r.blocks, { anchor: r.pos, focus: r.pos });
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
