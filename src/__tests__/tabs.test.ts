import { describe, expect, it } from "vitest";
import { TabManager, type TabEvent } from "../core/tabs";

const tab = (docId: number, mode: "document" | "workbook" | "presentation" = "document", path: string | null = null) => ({
  docId,
  mode,
  title: `T${docId}`,
  path,
});

describe("TabManager", () => {
  it("activates newly opened tabs and emits events", () => {
    const tm = new TabManager();
    const events: TabEvent["type"][] = [];
    tm.on((e) => events.push(e.type));
    tm.open(tab(1));
    tm.open(tab(2, "workbook"));
    expect(tm.active?.docId).toBe(2);
    expect(events).toEqual(["opened", "activated", "opened", "activated"]);
  });

  it("re-opening an existing doc only activates it", () => {
    const tm = new TabManager();
    tm.open(tab(1));
    tm.open(tab(2));
    tm.open(tab(1));
    expect(tm.list()).toHaveLength(2);
    expect(tm.active?.docId).toBe(1);
  });

  it("closing the active tab falls back to the most recently used", () => {
    const tm = new TabManager();
    tm.open(tab(1));
    tm.open(tab(2));
    tm.open(tab(3));
    tm.activate(1);
    tm.close(1);
    expect(tm.active?.docId).toBe(3);
    tm.close(3);
    expect(tm.active?.docId).toBe(2);
    tm.close(2);
    expect(tm.active).toBeNull();
  });

  it("cycles in strip order with wrap-around", () => {
    const tm = new TabManager();
    [1, 2, 3].forEach((i) => tm.open(tab(i)));
    tm.cycle(1);
    expect(tm.active?.docId).toBe(1);
    tm.cycle(-1);
    expect(tm.active?.docId).toBe(3);
  });

  it("finds tabs by path regardless of separators and case", () => {
    const tm = new TabManager();
    tm.open(tab(7, "presentation", "C:\\Decks\\Pitch.pptx"));
    expect(tm.findByPath("c:/decks/pitch.PPTX")?.docId).toBe(7);
  });

  it("update patches and emits", () => {
    const tm = new TabManager();
    tm.open(tab(1));
    let last: TabEvent | undefined;
    tm.on((e) => (last = e));
    tm.update(1, { dirty: true });
    expect(tm.get(1)?.dirty).toBe(true);
    expect(last?.type).toBe("updated");
  });
});
