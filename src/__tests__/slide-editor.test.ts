import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { beforeAll, describe, expect, it, vi } from "vitest";
import type { HostBridge } from "../core/host";
import { SlideEditor, resizeXfrm } from "../slide/editor";
import { evaluate, loadDrawingmlSync, Palette } from "../slide/engine";
import { blankPresentation, type PresentationModel, type SlideNode } from "../slide/model";
import { applyLayout, childToSlide, group, newShape, slideFromLayout, slideText, ungroup, zOrder } from "../slide/ops";
import { effectivePara, matchPlaceholder, resolveShape, slideContext } from "../slide/resolve";
import { readEditable, renderEditable } from "../slide/textedit";
import { autoNumber, layoutText } from "../slide/text";

beforeAll(() => {
  loadDrawingmlSync(readFileSync(resolve(__dirname, "../slide/wasm/prism_drawingml_bg.wasm")));
  HTMLCanvasElement.prototype.getContext = (() => null) as never;
});

const sq = (x = 0, y = 0, c = 914400) => ({ x, y, cx: c, cy: c });

describe("drawingml engine", () => {
  it("evaluates presets and resolves theme colours", () => {
    const e = evaluate({ t: "preset", name: "rect" }, 100, 50);
    expect(e.paths[0]!.cmds[0]).toEqual({ c: "M", x: 0, y: 0 });
    const m = blankPresentation();
    const p = new Palette(m.masters[0]!);
    expect(p.css({ base: { t: "scheme", v: "accent1" } }).toLowerCase()).toBe("#c2553a");
    expect(p.css({ base: { t: "scheme", v: "tx1" } })).toBe("#000000");
    expect(p.css({ base: { t: "scheme", v: "phClr" } }, "#336699")).toBe("#336699");
  });
});

describe("placeholder inheritance", () => {
  it("resolves title position and text style from layout and master", () => {
    const m = blankPresentation();
    const slide = m.slides[0]!;
    const ctx = slideContext(m, slide);
    expect(ctx.layout.name).toBe("Title Slide");
    const title = slide.shapes.find((n) => n.ph?.kind === "ctrTitle")!;
    const res = resolveShape(title, ctx, m);
    expect(res.xfrm).toMatchObject({ x: 1524000, y: 1122363 });
    expect(res.body.anchor).toBe("b");
    const pp = effectivePara(res.lists, undefined);
    expect(pp.align).toBe("ctr");
    expect(pp.run?.size).toBe(6000);
    expect(pp.run?.font).toBe("+mj-lt");
  });

  it("matches body placeholders by idx and maps ctrTitle to title on the master", () => {
    const m = blankPresentation();
    const layout = m.masters[0]!.layouts[3]!; // Two Content
    expect(matchPlaceholder({ idx: 2 }, layout.shapes, false)?.name).toContain("Content");
    expect(matchPlaceholder({ kind: "ctrTitle" }, m.masters[0]!.shapes, true)?.ph?.kind).toBe("title");
  });
});

describe("slide ops", () => {
  it("creates slides from layouts and keeps content on layout change", () => {
    const m = blankPresentation();
    const [, content, , two] = m.masters[0]!.layouts;
    const s = slideFromLayout(content!);
    expect(s.shapes.map((n) => n.ph)).toEqual([{ kind: "title" }, { idx: 1 }]);
    (s.shapes[0] as Extract<SlideNode, { type: "shape" }>).text = { paragraphs: [{ runs: [{ t: "text", text: "Kept" }] }] };
    const s2 = applyLayout(s, two!);
    expect(s2.layout).toBe(two!.part);
    expect(s2.shapes).toHaveLength(3);
    expect(slideText(s2)).toBe("Kept");
  });

  it("groups and ungroups preserving positions; z-order", () => {
    const m = blankPresentation();
    const s = { layout: m.slides[0]!.layout, shapes: [] as SlideNode[], notes: "" };
    const a = newShape(s, "rect", sq(0, 0));
    s.shapes.push(a);
    const b = newShape(s, "ellipse", sq(1828800, 914400));
    s.shapes.push(b);
    zOrder(s, new Set([a.id]), "front");
    expect(s.shapes[1]!.id).toBe(a.id);
    const g = group(s, new Set([a.id, b.id]), m)!;
    expect(g.xfrm).toMatchObject({ x: 0, y: 0, cx: 2743200, cy: 1828800 });
    g.xfrm!.x += 100;
    const kids = ungroup(s, g.id);
    expect(kids.map((k) => k.xfrm!.x)).toEqual([1828900, 100]);
    expect(childToSlide({ x: 0, y: 0, cx: 200, cy: 200, child: [0, 0, 100, 100] }, { x: 50, y: 50, cx: 50, cy: 50 })).toMatchObject({ x: 100, y: 100, cx: 100, cy: 100 });
  });

  it("resizes from a corner keeping the opposite corner fixed, also when rotated", () => {
    const K = 1000;
    const o = { x: 0, y: 0, cx: 1000 * K, cy: 1000 * K };
    expect(resizeXfrm(o, 4, 500 * K, 200 * K, false)).toMatchObject({ x: 0, y: 0, cx: 1500 * K, cy: 1200 * K });
    expect(resizeXfrm(o, 0, 100 * K, 100 * K, false)).toMatchObject({ x: 100 * K, y: 100 * K, cx: 900 * K, cy: 900 * K });
    expect(resizeXfrm(o, 4, 1000 * K, 0, true)).toMatchObject({ cx: 2000 * K, cy: 2000 * K });
    // rotated 90°: dragging the local east handle by +y in slide space widens it
    const r = resizeXfrm({ ...o, rot: 5400000 }, 3, 0, 500 * K, false);
    expect(r.cx).toBe(1500 * K);
    expect(r.cy).toBe(1000 * K);
    expect(r.x + r.cx / 2).toBe(500 * K); // centre moved down only
    expect(r.y + r.cy / 2).toBe(750 * K);
  });
});

describe("text", () => {
  it("numbers lists and wraps text", () => {
    expect(autoNumber("arabicPeriod", 3)).toBe("3.");
    expect(autoNumber("romanUcPeriod", 4)).toBe("IV.");
    expect(autoNumber("alphaLcParenR", 28)).toBe("ab)");
    const m = blankPresentation();
    const ctx = slideContext(m, m.slides[0]!);
    const res = resolveShape(m.slides[0]!.shapes[0]!, ctx, m);
    const lay = layoutText({ paragraphs: [{ runs: [{ t: "text", text: "word ".repeat(40) }] }] }, res, 300, 400, { palette: new Palette(ctx.master), theme: ctx.master.theme });
    expect(lay.lines.length).toBeGreaterThan(3);
    expect(lay.lines.every((l) => l.items.every((g) => g.x + g.w <= lay.box.w + 0.01))).toBe(true);
  });

  it("round-trips a text body through the editable DOM, reading inline formatting", () => {
    const m = blankPresentation();
    const ctx = slideContext(m, m.slides[0]!);
    const res = resolveShape(m.slides[0]!.shapes[1]!, ctx, m);
    const env = { palette: new Palette(ctx.master), theme: ctx.master.theme, scale: 1 };
    const body = {
      body: { anchor: "ctr" },
      paragraphs: [
        { props: { align: "l" }, runs: [{ t: "text" as const, text: "Plain " }, { t: "text" as const, text: "red", props: { fill: { t: "solid" as const, color: { base: { t: "scheme" as const, v: "accent2" } } }, lang: "az-AZ" } }] },
        { runs: [{ t: "text" as const, text: "a" }, { t: "break" as const }, { t: "text" as const, text: "b" }] },
        {},
      ],
    };
    const root = document.createElement("div");
    renderEditable(root, body, res, env);
    expect(readEditable(root, body, env)).toEqual(body);
    // the browser wraps a selection in <b> / a CSS span
    const first = root.children[0]!;
    const b = document.createElement("b");
    b.textContent = "Bold";
    first.append(b);
    const span = document.createElement("span");
    span.setAttribute("style", "color: rgb(0, 112, 192); font-size: 32px");
    span.textContent = "Big";
    first.append(span);
    const out = readEditable(root, body, env);
    expect(out.paragraphs![0]!.runs!.slice(2)).toEqual([
      { t: "text", text: "Bold", props: { bold: true } },
      { t: "text", text: "Big", props: { fill: { t: "solid", color: { base: { t: "srgb", v: "0070C0" } } }, size: 3200 } },
    ]);
    expect(out.paragraphs![0]!.runs![1]).toEqual(body.paragraphs[0]!.runs![1]);
  });
});

describe("SlideEditor", () => {
  async function editor(model: PresentationModel = blankPresentation()) {
    const updates: PresentationModel[] = [];
    const host = {
      updatePresentationModel: vi.fn(async (_id: number, m: PresentationModel) => void updates.push(structuredClone(m))),
      getPresentationModel: vi.fn(),
      getSlidePart: vi.fn(async () => null),
      addSlideImage: vi.fn(async () => "/ppt/media/image1.png"),
      pickImage: vi.fn(async () => null),
    } as unknown as HostBridge;
    const dirty = vi.fn();
    const ed = new SlideEditor(host, 1, { model, onDirty: dirty });
    document.body.append(ed.el);
    await ed.ready;
    return { ed, host, updates, dirty };
  }

  it("adds, duplicates, reorders, hides and deletes slides with undo", async () => {
    const { ed, dirty } = await editor();
    ed.execute("newSlide");
    expect(ed.model!.slides).toHaveLength(2);
    expect(slideContext(ed.model!, ed.model!.slides[1]!).layout.name).toBe("Title and Content");
    ed.execute("duplicateSlide");
    expect(ed.model!.slides).toHaveLength(3);
    expect(ed.current).toBe(2);
    ed.execute("moveSlideUp");
    expect(ed.current).toBe(1);
    ed.execute("hideSlide");
    expect(ed.model!.slides[1]!.hidden).toBe(true);
    ed.execute("deleteSlide");
    expect(ed.model!.slides).toHaveLength(2);
    ed.undo();
    expect(ed.model!.slides).toHaveLength(3);
    ed.redo();
    expect(ed.model!.slides).toHaveLength(2);
    expect(dirty).toHaveBeenCalled();
  });

  it("formats selected shapes, groups them and pushes the model to the host", async () => {
    const { ed, updates } = await editor();
    const s = ed.slide!;
    for (const n of s.shapes) if (n.type === "shape") n.text = { paragraphs: [{ runs: [{ t: "text", text: n.name }] }] };
    ed.execute("selectAll");
    ed.execute("bold");
    expect(s.shapes.every((n) => n.type === "shape" && n.text!.paragraphs![0]!.runs!.every((r) => r.props?.bold))).toBe(true);
    expect(ed.state().bold).toBe(true);
    ed.execute("fontSize", "40");
    expect((s.shapes[0] as Extract<SlideNode, { type: "shape" }>).text!.paragraphs![0]!.runs![0]!.props!.size).toBe(4000);
    ed.execute("align", "r");
    expect(ed.state().align).toBe("r");
    ed.execute("group");
    expect(ed.slide!.shapes).toHaveLength(1);
    expect(ed.slide!.shapes[0]!.type).toBe("group");
    // placeholders became free shapes with an explicit position
    const g = ed.slide!.shapes[0] as Extract<SlideNode, { type: "group" }>;
    expect(g.children.every((c) => !c.ph && c.xfrm)).toBe(true);
    await ed.flush();
    expect(updates.at(-1)!.slides[0]!.shapes[0]!.type).toBe("group");
  });

  it("edits placeholder text in place", async () => {
    const { ed } = await editor();
    const title = ed.slide!.shapes[0]!;
    ed.selection.add(title.id);
    ed.startEdit(title.id, { replaceWith: "N" });
    const root = ed.el.querySelector<HTMLElement>(".slide-text-edit")!;
    expect(root).toBeTruthy();
    root.querySelector("span")!.textContent = "New title";
    root.dispatchEvent(new Event("input"));
    ed.commitEdit();
    expect(slideText(ed.slide!)).toBe("New title");
    expect(ed.el.querySelector(".slide-text-edit")).toBeNull();
    ed.undo();
    expect(slideText(ed.slide!)).toBe("");
  });

  it("copies, pastes with offset, deletes and changes slide size", async () => {
    const { ed } = await editor();
    const s = ed.slide!;
    const shape = newShape(s, "star5", sq(100, 100));
    s.shapes.push(shape);
    ed.selection.clear();
    ed.selection.add(shape.id);
    ed.execute("duplicate");
    expect(ed.slide!.shapes).toHaveLength(4);
    const dup = ed.slide!.shapes[3]!;
    expect(dup.id).not.toBe(shape.id);
    expect(dup.xfrm!.x).toBe(100 + 182880);
    ed.execute("delete");
    expect(ed.slide!.shapes).toHaveLength(3);
    ed.execute("slideSize", "4:3");
    expect(ed.model!.width).toBe(9144000);
    expect(ed.slide!.shapes[2]!.xfrm!.cx).toBe(Math.round(914400 * 0.75));
  });
});
