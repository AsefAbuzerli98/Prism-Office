import type { Mode } from "./modes";
import { modeFromPath } from "./modes";

export interface ValidationIssue {
  severity: "error" | "warning";
  part: string | null;
  message: string;
}

/** Mirrors `documents::DocumentInfo` in the Rust host. */
export interface DocumentInfo {
  id: number;
  kind: Mode;
  title: string;
  path: string | null;
  partCount: number;
  issues: ValidationIssue[];
}

export interface AppInfo {
  name: string;
  version: string;
  os: string;
  arch: string;
}

/** Everything the UI may ask of the native host. */
export interface HostBridge {
  readonly native: boolean;
  appInfo(): Promise<AppInfo>;
  newDocument(kind: Mode): Promise<DocumentInfo>;
  openDocument(path: string): Promise<DocumentInfo>;
  saveDocument(id: number, path?: string): Promise<DocumentInfo>;
  closeDocument(id: number): Promise<void>;
}

type Invoke = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;

/** Bridge backed by Tauri IPC commands (see `src-tauri/src/lib.rs`). */
export class TauriHost implements HostBridge {
  readonly native = true;
  constructor(private readonly invoke: Invoke) {}
  appInfo() {
    return this.invoke<AppInfo>("app_info");
  }
  newDocument(kind: Mode) {
    return this.invoke<DocumentInfo>("new_document", { kind });
  }
  openDocument(path: string) {
    return this.invoke<DocumentInfo>("open_document", { path });
  }
  saveDocument(id: number, path?: string) {
    return this.invoke<DocumentInfo>("save_document", { id, path: path ?? null });
  }
  closeDocument(id: number) {
    return this.invoke<void>("close_document", { id });
  }
}

/**
 * In-memory bridge used when running in a plain browser (`pnpm dev` without
 * Tauri) and in unit tests. Behaviour mirrors the Rust registry.
 */
export class BrowserHost implements HostBridge {
  readonly native = false;
  private nextId = 0;
  private docs = new Map<number, DocumentInfo>();
  private untitled: Record<Mode, number> = { document: 0, workbook: 0, presentation: 0 };

  async appInfo(): Promise<AppInfo> {
    return { name: "Prism Office", version: "0.1.0-web", os: "browser", arch: "wasm" };
  }

  async newDocument(kind: Mode): Promise<DocumentInfo> {
    const n = ++this.untitled[kind];
    const base = kind[0]!.toUpperCase() + kind.slice(1);
    return this.store({ kind, title: `${base} ${n}`, path: null });
  }

  async openDocument(path: string): Promise<DocumentInfo> {
    const kind = modeFromPath(path);
    if (!kind) throw new Error(`unsupported file type: ${path}`);
    return this.store({ kind, title: path.split(/[\\/]/).pop()!, path });
  }

  async saveDocument(id: number, path?: string): Promise<DocumentInfo> {
    const doc = this.docs.get(id);
    if (!doc) throw new Error(`no open document with id ${id}`);
    const target = path ?? doc.path;
    if (!target) throw new Error("document has never been saved; a path is required");
    if (modeFromPath(target) !== doc.kind) {
      throw new Error(`file extension does not match a ${doc.kind}`);
    }
    doc.path = target;
    doc.title = target.split(/[\\/]/).pop()!;
    return { ...doc };
  }

  async closeDocument(id: number): Promise<void> {
    if (!this.docs.delete(id)) throw new Error(`no open document with id ${id}`);
  }

  private store(d: Pick<DocumentInfo, "kind" | "title" | "path">): DocumentInfo {
    const info: DocumentInfo = { id: ++this.nextId, partCount: 0, issues: [], ...d };
    this.docs.set(info.id, info);
    return { ...info };
  }
}

/** Pick the Tauri bridge when running inside the native shell. */
export async function detectHost(): Promise<HostBridge> {
  const w = globalThis as { __TAURI_INTERNALS__?: unknown };
  if (w.__TAURI_INTERNALS__) {
    const { invoke } = await import("@tauri-apps/api/core");
    return new TauriHost(invoke as Invoke);
  }
  return new BrowserHost();
}
