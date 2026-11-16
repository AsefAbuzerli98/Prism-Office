/**
 * Browser preview file I/O: the host's DOCX/XLSX/PPTX readers and writers
 * compiled to WASM (crates/prism-io, built by scripts/build-io-wasm.sh),
 * loaded only when the preview opens or saves a real file.
 */
import type { IoDocument } from "./io-wasm/prism_io.js";

type IoModule = typeof import("./io-wasm/prism_io.js");
let mod: Promise<IoModule> | null = null;

export function loadIo(): Promise<IoModule> {
  mod ??= import("./io-wasm/prism_io.js").then(async (m) => {
    await m.default();
    return m;
  });
  return mod;
}

export type { IoDocument };

/** Let the user choose an Office file; resolves to its name and bytes. */
export function pickOfficeFile(): Promise<{ name: string; bytes: Uint8Array } | null> {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".docx,.docm,.dotx,.xlsx,.xlsm,.xltx,.pptx,.pptm,.potx";
    input.onchange = () => {
      const f = input.files?.[0];
      if (!f) return resolve(null);
      f.arrayBuffer().then(
        (b) => resolve({ name: f.name, bytes: new Uint8Array(b) }),
        () => resolve(null),
      );
    };
    input.click();
  });
}

/** Hand bytes to the browser's download manager. */
export function download(name: string, bytes: Uint8Array) {
  const url = URL.createObjectURL(new Blob([bytes as BlobPart]));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 60_000);
}
