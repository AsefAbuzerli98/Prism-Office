import "./styles/app.css";
import { detectHost } from "./core/host";
import { Shell } from "./shell/shell";

async function boot() {
  const root = document.getElementById("app");
  if (!root) throw new Error("#app root missing");
  const host = await detectHost();
  // uncaught front-end errors go to the local diagnostics log (message + location only)
  window.addEventListener("error", (e) => host.logEvent("error", "window", `${e.message} @ ${e.filename}:${e.lineno}:${e.colno}`));
  window.addEventListener("unhandledrejection", (e) => host.logEvent("error", "promise", String(e.reason instanceof Error ? `${e.reason.message}\n${e.reason.stack ?? ""}` : e.reason).slice(0, 2000)));
  const shell = new Shell(root, host);
  await shell.init();
  (globalThis as { prism?: unknown }).prism = { shell, host };
}

void boot();
