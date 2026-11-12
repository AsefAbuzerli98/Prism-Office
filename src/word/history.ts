import type { Block } from "./model";
import type { Selection } from "./ops";

export interface Snapshot {
  blocks: Block[];
  selection: Selection;
}

interface Entry extends Snapshot {
  kind: string;
  at: number;
}

/**
 * Undo/redo over immutable block snapshots. Consecutive edits of the same
 * groupable kind (typing, deleting) within `groupMs` merge into one step.
 */
export class History {
  private undoStack: Entry[] = [];
  private redoStack: Snapshot[] = [];

  constructor(
    private readonly limit = 500,
    private readonly groupMs = 1000,
    private readonly now: () => number = () => Date.now(),
  ) {}

  /** Record the state *before* an edit. */
  record(before: Snapshot, kind: string) {
    const t = this.now();
    const last = this.undoStack[this.undoStack.length - 1];
    const groupable = kind === "type" || kind === "delete";
    if (!(groupable && last && last.kind === kind && t - last.at < this.groupMs)) {
      this.undoStack.push({ ...before, kind, at: t });
      if (this.undoStack.length > this.limit) this.undoStack.shift();
    } else {
      last.at = t;
    }
    this.redoStack = [];
  }

  /** Break the current typing group (e.g. after caret movement). */
  seal() {
    const last = this.undoStack[this.undoStack.length - 1];
    if (last) last.kind = `${last.kind}:sealed`;
  }

  get canUndo() {
    return this.undoStack.length > 0;
  }
  get canRedo() {
    return this.redoStack.length > 0;
  }

  undo(current: Snapshot): Snapshot | null {
    const e = this.undoStack.pop();
    if (!e) return null;
    this.redoStack.push(current);
    return { blocks: e.blocks, selection: e.selection };
  }

  redo(current: Snapshot): Snapshot | null {
    const s = this.redoStack.pop();
    if (!s) return null;
    this.undoStack.push({ ...current, kind: "redo", at: this.now() });
    return s;
  }
}
