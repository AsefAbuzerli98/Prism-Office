import { blankDocument, type WordDocument } from "../word/model";
import { blankWorkbook, type WorkbookModel } from "../sheet/model";
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

export interface RecoverableDoc {
  key: string;
  title: string;
  path: string | null;
  kind: Mode;
  /** Unix ms of the last journaled change. */
  updated: number;
  entries: number;
}

export interface AiStatus {
  configured: boolean;
  keySource: "keychain" | "file" | "environment" | "none";
  model: string;
  effort: string;
  /** Privacy switch: false = no document content is sent anywhere. */
  enabled: boolean;
  provider: string;
}

export interface AiRequest {
  system: string;
  prompt: string;
  images?: { mediaType: string; data: string }[];
  effort?: string;
  maxTokens?: number;
}

export interface AiResponse {
  text: string;
  stopReason: string;
  model: string;
  refusal?: string;
}

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

  /** Word-processing model of a document (Document mode). */
  getWordDocument(id: number): Promise<WordDocument>;
  updateWordDocument(id: number, content: WordDocument): Promise<void>;
  /** Spreadsheet model of a workbook (Workbook mode). */
  getSheetModel(id: number): Promise<WorkbookModel>;
  updateSheetModel(id: number, content: WorkbookModel): Promise<void>;
  /** Picture bytes (base64) for an image relationship. */
  getWordImage(id: number, relId: string): Promise<{ mime: string; data: string } | null>;
  /** Store a picture in the package; returns its relationship id and EMU size. */
  addWordImage(id: number, base64: string): Promise<{ relId: string; cx: number; cy: number }>;
  /** Fonts embedded in the document (base64 TrueType/OpenType). */
  getWordFonts(id: number): Promise<{ name: string; style: string; data: string }[]>;
  /** Let the user choose a picture file; resolves to base64 bytes or null. */
  pickImage(): Promise<string | null>;

  /** Documents with unsaved work journaled by an earlier session. */
  listRecoverable(): Promise<RecoverableDoc[]>;
  restoreRecovered(key: string): Promise<DocumentInfo>;
  discardRecovered(key: string): Promise<void>;

  aiStatus(): Promise<AiStatus>;
  setAiConfig(cfg: { apiKey?: string; model?: string; effort?: string; enabled?: boolean }): Promise<AiStatus>;
  /** One Claude request; `onDelta` receives streamed text. */
  aiComplete(req: AiRequest, onDelta?: (text: string) => void): Promise<AiResponse>;

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
  getWordDocument(id: number) {
    return this.api.invoke<WordDocument>("get_word_document", { id });
  }
  updateWordDocument(id: number, content: WordDocument) {
    return this.api.invoke<void>("update_word_document", { id, content });
  }
  getSheetModel(id: number) {
    return this.api.invoke<WorkbookModel>("get_sheet_model", { id });
  }
  updateSheetModel(id: number, content: WorkbookModel) {
    return this.api.invoke<void>("update_sheet_model", { id, content });
  }
  getWordImage(id: number, relId: string) {
    return this.api.invoke<{ mime: string; data: string } | null>("get_word_image", { id, relId });
  }
  addWordImage(id: number, data: string) {
    return this.api.invoke<{ relId: string; cx: number; cy: number }>("add_word_image", { id, data });
  }
  getWordFonts(id: number) {
    return this.api.invoke<{ name: string; style: string; data: string }[]>("get_word_fonts", { id });
  }
  listRecoverable() {
    return this.api.invoke<RecoverableDoc[]>("list_recoverable");
  }
  restoreRecovered(key: string) {
    return this.api.invoke<DocumentInfo>("restore_recovered", { key });
  }
  discardRecovered(key: string) {
    return this.api.invoke<void>("discard_recovered", { key });
  }
  aiStatus() {
    return this.api.invoke<AiStatus>("ai_status");
  }
  setAiConfig(cfg: { apiKey?: string; model?: string; effort?: string; enabled?: boolean }) {
    return this.api.invoke<AiStatus>("set_ai_config", { apiKey: cfg.apiKey ?? null, model: cfg.model ?? null, effort: cfg.effort ?? null, enabled: cfg.enabled ?? null });
  }
  async aiComplete(req: AiRequest, onDelta?: (text: string) => void) {
    const requestId = `ai-${Date.now()}-${Math.random().toString(36).slice(2)}`;
    const unlisten = onDelta
      ? await this.api.listen("prism://ai", (e) => {
          const p = e.payload as unknown as { requestId: string; delta: string };
          if (p.requestId === requestId) onDelta(p.delta);
        })
      : () => {};
    try {
      return await this.api.invoke<AiResponse>("ai_complete", { request: { requestId, ...req } });
    } finally {
      unlisten();
    }
  }
  pickImage() {
    return pickImageFile();
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

  private words = new Map<number, WordDocument>();

  async getWordDocument(id: number): Promise<WordDocument> {
    const doc = this.docs.get(id);
    if (!doc) throw new Error(`no open document with id ${id}`);
    if (doc.kind !== "document") throw new Error(`document ${id} is not a word-processing document`);
    let w = this.words.get(id);
    if (!w) {
      w = blankDocument();
      this.words.set(id, w);
    }
    return structuredClone(w);
  }

  async updateWordDocument(id: number, content: WordDocument): Promise<void> {
    if (!this.docs.has(id)) throw new Error(`no open document with id ${id}`);
    this.words.set(id, structuredClone(content));
  }

  private sheetModels = new Map<number, WorkbookModel>();

  async getSheetModel(id: number): Promise<WorkbookModel> {
    const doc = this.docs.get(id);
    if (!doc) throw new Error(`no open document with id ${id}`);
    if (doc.kind !== "workbook") throw new Error(`document ${id} is not a spreadsheet`);
    let m = this.sheetModels.get(id);
    if (!m) {
      m = blankWorkbook();
      this.sheetModels.set(id, m);
    }
    return structuredClone(m);
  }

  async updateSheetModel(id: number, content: WorkbookModel): Promise<void> {
    if (!this.docs.has(id)) throw new Error(`no open document with id ${id}`);
    this.sheetModels.set(id, structuredClone(content));
  }

  private images = new Map<string, { mime: string; data: string }>();
  async getWordImage(id: number, relId: string) {
    return this.images.get(`${id}:${relId}`) ?? null;
  }
  async addWordImage(id: number, data: string) {
    const info = imageInfo(data);
    const relId = `rIdImg${this.images.size + 1}`;
    this.images.set(`${id}:${relId}`, { mime: info.mime, data });
    let cx = info.w * 9525;
    let cy = info.h * 9525;
    if (cx > 5_486_400) {
      cy = Math.round((cy * 5_486_400) / cx);
      cx = 5_486_400;
    }
    return { relId, cx, cy };
  }
  async getWordFonts() {
    return [];
  }
  /** Recovery journals (tests / browser preview keep them in memory). */
  recoverable: RecoverableDoc[] = [];
  async listRecoverable() {
    return [...this.recoverable];
  }
  async restoreRecovered(key: string) {
    const r = this.recoverable.find((x) => x.key === key);
    if (!r) throw new Error("recovery journal not found");
    this.recoverable = this.recoverable.filter((x) => x.key !== key);
    return this.store({ kind: r.kind, title: `${r.title} (recovered)`, path: null });
  }
  async discardRecovered(key: string) {
    this.recoverable = this.recoverable.filter((x) => x.key !== key);
  }
  /** Test hook / dev stub for AI calls in the browser preview. */
  aiHandler: ((req: AiRequest, onDelta?: (t: string) => void) => Promise<AiResponse>) | null = null;
  async aiStatus(): Promise<AiStatus> {
    return { configured: !!this.aiHandler, keySource: "none", model: "claude-opus-5-5", effort: "high", enabled: true, provider: "anthropic" };
  }
  async setAiConfig(): Promise<AiStatus> {
    return this.aiStatus();
  }
  async aiComplete(req: AiRequest, onDelta?: (t: string) => void): Promise<AiResponse> {
    if (!this.aiHandler) throw new Error("AI features require the desktop app (the API key stays in the native host).");
    return this.aiHandler(req, onDelta);
  }
  pickImage(): Promise<string | null> {
    return pickImageFile();
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

/** Pixel size and MIME type from base64 PNG/JPEG/GIF bytes. */
export function imageInfo(base64: string): { w: number; h: number; mime: string } {
  const bin = atob(base64.slice(0, 64 * 1024));
  const b = (i: number) => bin.charCodeAt(i);
  if (bin.startsWith("\x89PNG")) return { w: (b(16) << 24) | (b(17) << 16) | (b(18) << 8) | b(19), h: (b(20) << 24) | (b(21) << 16) | (b(22) << 8) | b(23), mime: "image/png" };
  if (bin.startsWith("GIF8")) return { w: b(6) | (b(7) << 8), h: b(8) | (b(9) << 8), mime: "image/gif" };
  if (b(0) === 0xff && b(1) === 0xd8) {
    let i = 2;
    while (i + 9 < bin.length) {
      if (b(i) !== 0xff) { i++; continue; }
      const m = b(i + 1);
      const len = (b(i + 2) << 8) | b(i + 3);
      if (m >= 0xc0 && m <= 0xcf && m !== 0xc4 && m !== 0xc8 && m !== 0xcc) return { h: (b(i + 5) << 8) | b(i + 6), w: (b(i + 7) << 8) | b(i + 8), mime: "image/jpeg" };
      i += 2 + len;
    }
    return { w: 400, h: 300, mime: "image/jpeg" };
  }
  return { w: 400, h: 300, mime: "application/octet-stream" };
}

/** Open a file chooser for images and read the choice as base64. */
export function pickImageFile(): Promise<string | null> {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "image/png,image/jpeg,image/gif,image/webp,image/bmp";
    input.onchange = () => {
      const f = input.files?.[0];
      if (!f) return resolve(null);
      const r = new FileReader();
      r.onload = () => resolve(String(r.result).split(",")[1] ?? null);
      r.onerror = () => resolve(null);
      r.readAsDataURL(f);
    };
    input.click();
  });
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
