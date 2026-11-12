import { describe, expect, it, vi } from "vitest";
import { Canvas2DBackend, PageCache } from "../word/compositor";

const fakeCanvas = () => ({ width: 0, height: 0, getContext: () => ({ setTransform: vi.fn(), clearRect: vi.fn() }) }) as unknown as HTMLCanvasElement;

describe("PageCache", () => {
  it("rasterises a page only when its version or scale changes", () => {
    const cache = new PageCache(() => fakeCanvas());
    const paint = vi.fn();
    const a = cache.get(0, 1, 2, 100, 200, paint);
    const b = cache.get(0, 1, 2, 100, 200, paint);
    expect(a).toBe(b);
    expect(paint).toHaveBeenCalledTimes(1);
    cache.get(0, 2, 2, 100, 200, paint);
    cache.get(0, 2, 1, 100, 200, paint);
    expect(paint).toHaveBeenCalledTimes(3);
    expect(cache.rasterised).toBe(3);
    cache.get(5, 1, 1, 10, 10, paint);
    cache.trim(1);
    cache.get(5, 1, 1, 10, 10, paint);
    expect(paint).toHaveBeenCalledTimes(5);
  });
});

describe("Canvas2DBackend", () => {
  it("clears and draws tiles scaled by devicePixelRatio", () => {
    const calls: unknown[][] = [];
    const ctx = { setTransform: vi.fn(), fillRect: vi.fn(), drawImage: (...a: unknown[]) => calls.push(a), fillStyle: "" } as unknown as CanvasRenderingContext2D;
    const canvas = { width: 0, height: 0 } as HTMLCanvasElement;
    const b = new Canvas2DBackend(canvas, ctx);
    b.resize(100, 50, 2);
    expect([canvas.width, canvas.height]).toEqual([200, 100]);
    const src = {} as HTMLCanvasElement;
    b.draw([{ key: "p0", version: 1, source: src, x: 10, y: 5, w: 30, h: 20 }], [1, 1, 1]);
    expect(calls[0]).toEqual([src, 20, 10, 60, 40]);
  });
});
