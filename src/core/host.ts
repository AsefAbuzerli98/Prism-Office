import type { Mode } from "./modes";
import { MODES, modeFromPath } from "./modes";

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

export interface RecentFile {
  path: string;
  kind: Mode;
  openedAt: number;
  exists: boolean;
}

export type Windowing = "tabbed" | "multiInstance";
export type Appearance = "system" | "light" | "dark";

export interface Settings {
  windowing: Windowing;
  appearance: Appearance;
  locale: string | null;
}

export const DEFAULT_SETTINGS: Settings = { windowing: "tabbed", appearance: "system", locale: null };

export type ServiceState =
  | { state: "stopped" }
  | { state: "running" }
  | { state: "unavailable"; reason: string }
  | { state: "failed"; error: string };

export type ServiceInfo = { id: string; name: string } & ServiceState;

/** Mirrors `events::AppEvent`. */
export type AppEvent =
  | { type: "documentOpened"; info: DocumentInfo }
  | { type: "documentSaved"; info: DocumentInfo }
  | { type: "documentClosed"; id: number }
  | { type: "serviceChanged"; info: ServiceInfo }
  | { type: "openFiles"; paths: string[] }
  | { type: "settingsChanged"; settings: Settings }
  | { type: "recentChanged" };

/** Which native window this UI instance is running in. */
export interface WindowContext {
  label: string;
  /** Set for dedicated per-document windows (multi-instance topology). */
  docId: number | null;
  platform: string;
}

/** Everything the UI may ask of the native host. */
export interface HostBridge {
  readonly native: boolean;
  /** Whether separate native document windows can be opened. */
  readonly multiWindow: boolean;
  windowContext(): WindowContext;
  appInfo(): Promise<AppInfo>;

  newDocument(kind: Mode): Promise<DocumentInfo>;
  openDocument(path: string): Promise<DocumentInfo>;
  getDocument(id: number): Promise<DocumentInfo>;
  saveDocument(id: number, path?: string): Promise<DocumentInfo>;
  closeDocument(id: number): Promise<void>;

  openDocumentWindow(id: number): Promise<void>;
  closeCurrentWindow(): Promise<void>;
  takePendingOpens(): Promise<string[]>;

  pickOpenPath(): Promise<string | null>;
  pickSavePath(kind: Mode, suggested: string): Promise<string | null>;

  recentFiles(): Promise<RecentFile[]>;
  removeRecent(path: string): Promise<void>;
  clearRecent(): Promise<void>;

  getSettings(): Promise<Settings>;
  setSettings(value: Settings): Promise<Settings>;
  listServices(): Promise<ServiceInfo[]>;

  /** Subscribe to host broadcasts; returns an unsubscribe function. */
  onEvent(listener: (e: AppEvent) => void): () => void;
}

type Invoke = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;

export interface TauriApis {
  invoke: Invoke;
  listen: (event: string, cb: (e: { payload: AppEvent }) => void) => Promise<() => void>;
  openDialog: (opts: Record<string, unknown>) => Promise<string | string[] | null>;
  saveDialog: (opts: Record<string, unknown>) => Promise<string | null>;
  closeWindow: () => Promise<void>;
}

const FILE_FILTERS = [
  { name: "Office files", extensions: [...MODES.document.extensions, ...MODES.workbook.extensions, ...MODES.presentation.extensions] },
  { name: "Documents", extensions: [...MODES.document.extensions] },
  { name: "Workbooks", extensions: [...MODES.workbook.extensions] },
  { name: "Presentations", extensions: [...MODES.presentation.extensions] },
];

function injectedContext(): WindowContext {
  const w = globalThis as { __PRISM_WINDOW__?: Partial<WindowContext> };
  const c = w.__PRISM_WINDOW__ ?? {};
  return { label: c.label ?? "main", docId: c.docId ?? null, platform: c.platform ?? "browser" };
}

/** Bridge backed by Tauri IPC commands (see `src-tauri/src/lib.rs`). */
export class TauriHost implements HostBridge {
  readonly native = true;
  readonly multiWindow = true;
  private listeners = new Set<(e: AppEvent) => void>();
  private subscribed = false;

  constructor(private readonly api: TauriApis) {}

  windowContext() {
    return injectedContext();
  }
  appInfo() {
    return this.api.invoke<AppInfo>("app_info");
  }
  newDocument(kind: Mode) {
    return this.api.invoke<DocumentInfo>("new_document", { kind });
  }
  openDocument(path: string) {
    return this.api.invoke<DocumentInfo>("open_document", { path });
  }
  getDocument(id: number) {
    return this.api.invoke<DocumentInfo>("get_document", { id });
  }
  saveDocument(id: number, path?: string) {
    return this.api.invoke<DocumentInfo>("save_document", { id, path: path ?? null });
  }
  closeDocument(id: number) {
    return this.api.invoke<void>("close_document", { id });
  }
  openDocumentWindow(id: number) {
    return this.api.invoke<void>("open_document_window", { id });
  }
  closeCurrentWindow() {
    return this.api.closeWindow();
  }
  takePendingOpens() {
    return this.api.invoke<string[]>("take_pending_opens");
  }
  async pickOpenPath() {
    const r = await this.api.openDialog({ multiple: false, directory: false, filters: FILE_FILTERS });
    return Array.isArray(r) ? (r[0] ?? null) : r;
  }
  pickSavePath(kind: Mode, suggested: string) {
    const info = MODES[kind];
    return this.api.saveDialog({
      defaultPath: suggested,
      filters: [{ name: info.label, extensions: [...info.extensions] }],
    });
  }
  recentFiles() {
    return this.api.invoke<RecentFile[]>("recent_files");
  }
  removeRecent(path: string) {
    return this.api.invoke<void>("remove_recent", { path });
  }
  clearRecent() {
    return this.api.invoke<void>("clear_recent");
  }
  getSettings() {
    return this.api.invoke<Settings>("get_settings");
  }
  setSettings(value: Settings) {
    return this.api.invoke<Settings>("set_settings", { value });
  }
  listServices() {
    return this.api.invoke<ServiceInfo[]>("list_services");
  }
  onEvent(listener: (e: AppEvent) => void) {
    this.listeners.add(listener);
    if (!this.subscribed) {
      this.subscribed = true;
      void this.api.listen("prism://event", (e) => {
        for (const l of this.listeners) l(e.payload);
      });
    }
    return () => this.listeners.delete(listener);
  }
}

/**
 * In-memory bridge used when running in a plain browser (`pnpm dev` without
 * Tauri) and in unit tests. Behaviour mirrors the Rust host; recents and
 * settings persist to `localStorage`.
 */
export class BrowserHost implements HostBridge {
  readonly native: boolean = false;
  readonly multiWindow: boolean = false;
  private nextId = 0;
  private docs = new Map<number, DocumentInfo>();
  private untitled: Record<Mode, number> = { document: 0, workbook: 0, presentation: 0 };
  private listeners = new Set<(e: AppEvent) => void>();

  constructor(
    private readonly storage: Storage = localStorage,
    /** Path prompt used in place of native dialogs. */
    private readonly ask: (message: string, value: string) => string | null = (m, v) => window.prompt(m, v),
  ) {}

  windowContext() {
    return injectedContext();
  }

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
    const info = this.store({ kind, title: path.split(/[\\/]/).pop()!, path });
    this.touchRecent(path, kind);
    return info;
  }

  async getDocument(id: number): Promise<DocumentInfo> {
    const doc = this.docs.get(id);
    if (!doc) throw new Error(`no open document with id ${id}`);
    return { ...doc };
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
    this.touchRecent(target, doc.kind);
    this.emit({ type: "documentSaved", info: { ...doc } });
    return { ...doc };
  }

  async closeDocument(id: number): Promise<void> {
    if (!this.docs.delete(id)) throw new Error(`no open document with id ${id}`);
    this.emit({ type: "documentClosed", id });
  }

  async openDocumentWindow(_id: number): Promise<void> {
    throw new Error("Separate windows are only available in the desktop app");
  }

  async closeCurrentWindow(): Promise<void> {}

  async takePendingOpens(): Promise<string[]> {
    return [];
  }

  async pickOpenPath() {
    return this.ask("Open file (full path):", "")?.trim() || null;
  }

  async pickSavePath(_kind: Mode, suggested: string) {
    return this.ask("Save as (full path):", suggested)?.trim() || null;
  }

  async recentFiles(): Promise<RecentFile[]> {
    try {
      const v = JSON.parse(this.storage.getItem("prism.recentFiles") ?? "[]");
      return Array.isArray(v) ? (v as RecentFile[]) : [];
    } catch {
      return [];
    }
  }

  async removeRecent(path: string) {
    const list = (await this.recentFiles()).filter((r) => r.path !== path);
    this.storage.setItem("prism.recentFiles", JSON.stringify(list));
    this.emit({ type: "recentChanged" });
  }

  async clearRecent() {
    this.storage.removeItem("prism.recentFiles");
    this.emit({ type: "recentChanged" });
  }

  async getSettings(): Promise<Settings> {
    try {
      return { ...DEFAULT_SETTINGS, ...JSON.parse(this.storage.getItem("prism.settings") ?? "{}") };
    } catch {
      return { ...DEFAULT_SETTINGS };
    }
  }

  async setSettings(value: Settings): Promise<Settings> {
    this.storage.setItem("prism.settings", JSON.stringify(value));
    this.emit({ type: "settingsChanged", settings: value });
    return value;
  }

  async listServices(): Promise<ServiceInfo[]> {
    return [
      { id: "ai", name: "AI services", state: "unavailable", reason: "desktop app only" },
      { id: "python", name: "Python sandbox", state: "unavailable", reason: "desktop app only" },
      { id: "recovery", name: "Auto-recovery journal", state: "unavailable", reason: "desktop app only" },
    ];
  }

  onEvent(listener: (e: AppEvent) => void) {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  /** Simulate the OS handing us files (tests / dev tools). */
  simulateOpenFiles(paths: string[]) {
    this.emit({ type: "openFiles", paths });
  }

  private emit(e: AppEvent) {
    for (const l of this.listeners) l(e);
  }

  private touchRecent(path: string, kind: Mode) {
    let list: RecentFile[] = [];
    try {
      list = JSON.parse(this.storage.getItem("prism.recentFiles") ?? "[]");
    } catch {
      /* ignore */
    }
    list = [{ path, kind, openedAt: Date.now(), exists: true }, ...list.filter((r) => r.path !== path)].slice(0, 20);
    this.storage.setItem("prism.recentFiles", JSON.stringify(list));
    this.emit({ type: "recentChanged" });
  }

  private store(d: Pick<DocumentInfo, "kind" | "title" | "path">): DocumentInfo {
    const info: DocumentInfo = { id: ++this.nextId, partCount: 0, issues: [], ...d };
    this.docs.set(info.id, info);
    this.emit({ type: "documentOpened", info: { ...info } });
    return { ...info };
  }
}

/** Pick the Tauri bridge when running inside the native shell. */
export async function detectHost(): Promise<HostBridge> {
  const w = globalThis as { __TAURI_INTERNALS__?: unknown };
  if (w.__TAURI_INTERNALS__) {
    const [{ invoke }, { listen }, dialog, { getCurrentWindow }] = await Promise.all([
      import("@tauri-apps/api/core"),
      import("@tauri-apps/api/event"),
      import("@tauri-apps/plugin-dialog"),
      import("@tauri-apps/api/window"),
    ]);
    return new TauriHost({
      invoke: invoke as Invoke,
      listen: listen as TauriApis["listen"],
      openDialog: dialog.open as TauriApis["openDialog"],
      saveDialog: dialog.save as TauriApis["saveDialog"],
      closeWindow: () => getCurrentWindow().close(),
    });
  }
  return new BrowserHost();
}
