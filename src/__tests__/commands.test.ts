import { describe, expect, it, vi } from "vitest";
import { CommandRegistry, formatKeybinding, fuzzyScore, matchesKeybinding, parseKeybinding } from "../core/commands";

const key = (k: string, mods: Partial<Pick<KeyboardEvent, "metaKey" | "ctrlKey" | "shiftKey" | "altKey">> = {}) =>
  new KeyboardEvent("keydown", { key: k, cancelable: true, ...mods });

describe("keybindings", () => {
  it("parses and matches Mod per platform", () => {
    const kb = parseKeybinding("Mod+Shift+S");
    expect(kb).toMatchObject({ mod: true, shift: true, key: "s" });
    expect(matchesKeybinding(key("S", { metaKey: true, shiftKey: true }), kb, true)).toBe(true);
    expect(matchesKeybinding(key("S", { ctrlKey: true, shiftKey: true }), kb, true)).toBe(false);
    expect(matchesKeybinding(key("s", { ctrlKey: true, shiftKey: true }), kb, false)).toBe(true);
    expect(matchesKeybinding(key("s", { ctrlKey: true }), kb, false)).toBe(false);
  });

  it("formats for display", () => {
    expect(formatKeybinding("Mod+Shift+S", true)).toBe("⇧⌘S");
    expect(formatKeybinding("Mod+Shift+S", false)).toBe("Ctrl+Shift+S");
    expect(formatKeybinding("Ctrl+Tab", true)).toBe("⌃Tab");
  });
});

describe("fuzzyScore", () => {
  it("matches subsequences and prefers word starts", () => {
    expect(fuzzyScore("nw", "New Workbook")).not.toBeNull();
    expect(fuzzyScore("xyz", "New Workbook")).toBeNull();
    expect(fuzzyScore("sa", "Save As…")!).toBeGreaterThan(fuzzyScore("sa", "Show Application Hub")!);
    expect(fuzzyScore("set", "Settings")!).toBeGreaterThan(fuzzyScore("set", "Save Extra Text")!);
    expect(fuzzyScore("", "anything")).toBe(0);
  });
});

describe("CommandRegistry", () => {
  it("dispatches keys respecting `when` and ranks search", async () => {
    const reg = new CommandRegistry(false);
    const save = vi.fn();
    let enabled = false;
    reg.register(
      { id: "save", title: () => "Save", keys: ["Mod+S"], when: () => enabled, run: save },
      { id: "settings", title: () => "Settings", run: () => {} },
      { id: "secret", title: () => "Secret Save", hidden: true, run: () => {} },
    );
    const e1 = key("s", { ctrlKey: true });
    expect(reg.handleKeydown(e1)).toBe(false);
    enabled = true;
    const e2 = key("s", { ctrlKey: true });
    expect(reg.handleKeydown(e2)).toBe(true);
    expect(e2.defaultPrevented).toBe(true);
    expect(save).toHaveBeenCalledOnce();
    expect(reg.search("sa").map((c) => c.id)).toEqual(["save"]);
    expect(await reg.execute("nope")).toBe(false);
  });
});
