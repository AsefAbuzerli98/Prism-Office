import { describe, expect, it } from "vitest";
import { checkAccessibility, readability } from "../word/a11y";
import { recognizeShape, shapeInline, simplify, type Pt } from "../word/ink";
import { blankDocument, newTable, paragraph, type WordDocument } from "../word/model";

const circle = (cx: number, cy: number, r: number, n = 60): Pt[] => Array.from({ length: n + 1 }, (_, i) => ({ x: cx + r * Math.cos((i / n) * 2 * Math.PI), y: cy + r * Math.sin((i / n) * 2 * Math.PI) }));
const poly = (pts: Pt[], per = 15): Pt[] => {
  const out: Pt[] = [];
  const ring = [...pts, pts[0]!];
  for (let i = 0; i < ring.length - 1; i++) for (let k = 0; k < per; k++) out.push({ x: ring[i]!.x + ((ring[i + 1]!.x - ring[i]!.x) * k) / per + Math.sin(k) * 0.8, y: ring[i]!.y + ((ring[i + 1]!.y - ring[i]!.y) * k) / per });
  out.push(pts[0]!);
  return out;
};

describe("ink recognition", () => {
  it("recognises circles, rectangles, triangles, diamonds, lines and arrows", () => {
    expect(recognizeShape(circle(100, 100, 40))?.preset).toBe("ellipse");
    expect(recognizeShape(poly([{ x: 0, y: 0 }, { x: 120, y: 0 }, { x: 120, y: 70 }, { x: 0, y: 70 }]))?.preset).toBe("rect");
    expect(recognizeShape(poly([{ x: 50, y: 0 }, { x: 100, y: 90 }, { x: 0, y: 90 }]))?.preset).toBe("triangle");
    expect(recognizeShape(poly([{ x: 50, y: 0 }, { x: 100, y: 50 }, { x: 50, y: 100 }, { x: 0, y: 50 }]))?.preset).toBe("diamond");
    expect(recognizeShape(Array.from({ length: 30 }, (_, i) => ({ x: i * 5, y: i * 2 + (i % 2) * 0.5 })))?.preset).toBe("line");
    const arrow: Pt[] = [...Array.from({ length: 40 }, (_, i) => ({ x: i * 5, y: 50 })), ...Array.from({ length: 8 }, (_, i) => ({ x: 195 - i * 3, y: 50 - i * 3 }))];
    expect(recognizeShape(arrow)?.preset).toBe("rightArrow");
    expect(recognizeShape([{ x: 0, y: 0 }, { x: 2, y: 1 }])).toBeNull();
  });

  it("simplifies polylines and builds shape inlines in EMU", () => {
    expect(simplify([{ x: 0, y: 0 }, { x: 5, y: 0.1 }, { x: 10, y: 0 }], 1)).toHaveLength(2);
    expect(shapeInline({ preset: "ellipse", w: 96, h: 48 })).toMatchObject({ type: "shape", preset: "ellipse", cx: 914400, cy: 457200 });
  });
});

describe("accessibility checker", () => {
  it("reports missing alt text, heading jumps, contrast, table headers and vague links", () => {
    const doc: WordDocument = {
      ...blankDocument(),
      blocks: [
        paragraph("Title", { style: "Heading1" }),
        paragraph("Deep", { style: "Heading3" }),
        { type: "paragraph", inlines: [{ type: "image", relId: "rId1", cx: 1, cy: 1, descr: "", xml: "" }, { type: "text", text: "faint", marks: { color: "DDDDDD" } }] },
        { type: "paragraph", inlines: [{ type: "text", text: "click here", marks: { link: "https://x.y" } }] },
        newTable(2, 2),
        paragraph("", { style: "Heading2" }),
      ],
    };
    const kinds = checkAccessibility(doc).map((i) => i.kind);
    expect(kinds).toEqual(expect.arrayContaining(["heading-order", "alt-text", "contrast", "link-text", "table-header", "empty-heading"]));
    const alt = checkAccessibility(doc).find((i) => i.kind === "alt-text")!;
    expect(alt).toMatchObject({ relId: "rId1", pos: { block: 2, offset: 0 } });
    expect(checkAccessibility(blankDocument())).toEqual([]);
  });

  it("scores readability", () => {
    const easy = readability({ ...blankDocument(), blocks: [paragraph("The cat sat. The dog ran. We had fun.")] });
    const hard = readability({ ...blankDocument(), blocks: [paragraph("Notwithstanding considerable organisational heterogeneity, implementation methodologies necessitate comprehensive interdepartmental coordination.")] });
    expect(easy.flesch).toBeGreaterThan(hard.flesch);
    expect(easy.lix).toBeLessThan(hard.lix);
    expect(easy.grade).toBe("very easy");
    expect(hard.grade).toBe("very difficult");
  });
});
