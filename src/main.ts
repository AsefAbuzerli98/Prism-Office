import "./styles/app.css";
import { detectHost } from "./core/host";
import { Shell } from "./shell/shell";

async function boot() {
  const root = document.getElementById("app");
  if (!root) throw new Error("#app root missing");
  const host = await detectHost();
  const shell = new Shell(root, host);
  await shell.init();
  (globalThis as { prism?: unknown }).prism = { shell, host };
}

void boot();
