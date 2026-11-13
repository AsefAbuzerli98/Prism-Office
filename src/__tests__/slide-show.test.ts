import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { beforeAll, describe, expect, it } from "vitest";
import { buildSteps, evaluate, pathPoint } from "../slide/anim";
import { loadDrawingmlSync } from "../slide/engine";
import { blankPresentation, type Effect, type SlideNode } from "../slide/model";
import { SlideShow } from "../slide/show";
import { morphPairs, transitionDuration } from "../slide/transition";

beforeAll(() => {
  loadDrawingmlSync(readFileSync(resolve(__dirname, "../slide/wasm/prism_drawingml_bg.wasm")));
  HTMLCanvasElement.prototype.getContext = (() => null) as never;
});

const fx = (shape: number, cls: string, preset: string, trigger: string, duration = 500, delay = 0): Effect => ({ shape, class: cls, preset, presetId: 10, trigger, duration, delay });
const box = () => ({ slideW: 960, slideH: 540, x: 100, y: 100, w: 200, h: 100 });

describe("animation timeline", () => {
  it("groups effects into click steps with PowerPoint timing", () => {
    const steps = buildSteps({
      effects: [fx(2, "entr", "fade", "withPrev", 400), fx(3, "entr", "fly", "onClick", 500), fx(4, "entr", "fade", "withPrev", 500, 100), fx(5, "emph", "spin", "afterPrev", 1000)],
    });
    expect(steps).toHaveLength(2);
    expect(steps[0]!.auto).toBe(true);
    expect(steps[1]!.items.map((i) => [i.begin, i.end])).toEqual([
      [0, 500],
      [100, 600],
      [600, 1600],
    ]);
    expect(steps[1]!.duration).toBe(1600);
  });

  it("hides entrance shapes until their step and animates them in", () => {
    const steps = buildSteps({ effects: [fx(3, "entr", "fade", "onClick"), fx(3, "exit", "fly", "onClick"), fx(4, "emph", "grow", "onClick")] });
    expect(evaluate(steps, -1, 0, box).get(3)).toEqual({ hidden: true });
    expect(evaluate(steps, 0, 250, box).get(3)!.opacity).toBeGreaterThan(0.5);
    expect(evaluate(steps, 0, Infinity, box).get(3)).toEqual({});
    const flying = evaluate(steps, 1, 250, box).get(3)!;
    expect(flying.dy).toBeGreaterThan(0);
    expect(evaluate(steps, 1, Infinity, box).get(3)).toEqual({ hidden: true });
    expect(evaluate(steps, 2, Infinity, box).get(4)!.scale).toBeCloseTo(1.5);
  });

  it("follows motion paths", () => {
    expect(pathPoint("M 0 0 L 0.5 0 L 0.5 0.5 E", 0.5)).toEqual([0.5, 0]);
    expect(pathPoint("M 0 0 L 1 0 E", 0.25)).toEqual([0.25, 0]);
    const steps = buildSteps({ effects: [{ ...fx(2, "path", "path", "onClick", 1000), path: "M 0 0 L 0.1 0 E" }] });
    expect(evaluate(steps, 0, Infinity, box).get(2)!.dx).toBeCloseTo(96);
  });
});

describe("transitions", () => {
  it("matches Morph shapes by name, picture, text and placeholder", () => {
    const a: SlideNode[] = [
      { type: "shape", id: 2, name: "Circle", xfrm: { x: 0, y: 0, cx: 10, cy: 10 } },
      { type: "picture", id: 3, name: "Pic A", image: { t: "image", relId: "/ppt/media/image1.png" } },
      { type: "shape", id: 4, name: "T1", text: { paragraphs: [{ runs: [{ t: "text", text: "Hello" }] }] } },
      { type: "shape", id: 5, name: "Gone" },
    ];
    const b: SlideNode[] = [
      { type: "shape", id: 9, name: "T2", text: { paragraphs: [{ runs: [{ t: "text", text: "Hello" }] }] } },
      { type: "picture", id: 8, name: "Pic B", image: { t: "image", relId: "/ppt/media/image1.png" } },
      { type: "shape", id: 7, name: "Circle", xfrm: { x: 100, y: 0, cx: 10, cy: 10 } },
      { type: "shape", id: 6, name: "New" },
    ];
    expect(morphPairs(a, b).sort()).toEqual([
      [0, 2],
      [1, 1],
      [2, 0],
    ]);
    expect(transitionDuration({ kind: "morph", advanceClick: true })).toBe(2000);
    expect(transitionDuration({ kind: "cut", advanceClick: true })).toBe(0);
  });

  it("steps through builds and slides, skipping hidden slides", async () => {
    const m = blankPresentation();
    const s0 = m.slides[0]!;
    s0.timing = { effects: [fx(2, "entr", "fade", "onClick"), fx(3, "entr", "fade", "onClick")] };
    m.slides.push({ ...structuredClone(s0), part: undefined, timing: undefined, hidden: true }, { ...structuredClone(s0), part: undefined, timing: undefined });
    let exited = -1;
    const show = new SlideShow(m, { fullscreen: false, onExit: (i) => (exited = i) });
    await show.start();
    expect(show.order).toEqual([0, 2]);
    show.next();
    expect(show.step).toBe(0);
    show.next(); // finish the running fade
    show.next();
    expect(show.step).toBe(1);
    show.next();
    show.next();
    expect(show.slideIndex).toBe(2);
    show.prev();
    expect(show.slideIndex).toBe(0);
    expect(show.step).toBe(1);
    show.next();
    show.next();
    show.next();
    expect(show.ended).toBe(true);
    show.next();
    expect(exited).toBe(2);
    expect(document.querySelector(".slideshow")).toBeNull();
  });
});
