/** PivotTable dialog: picks fields from the selected table and inserts a
 * dynamic-array PIVOTBY/GROUPBY formula (live, recalculates with the data). */
import { h } from "../core/dom";
import { pivotFormula, type Aggregate } from "./data";
import type { SheetEditor } from "./editor";

export function openPivotDialog(ed: SheetEditor): HTMLDialogElement | null {
  const wb = ed.wb;
  if (!wb) return null;
  const info = wb.sheets()[ed.sheet]!;
  let data = ed.selection;
  if ((data.r0 === data.r1 && data.c0 === data.c1) || data.r1 >= 1_048_575) data = info.used ?? data;
  if (data.r1 - data.r0 < 1) return null;
  const headers = Array.from({ length: data.c1 - data.c0 + 1 }, (_, i) => wb.input(ed.sheet, data.r0, data.c0 + i) || `Column ${i + 1}`);
  const select = (name: string, withNone: boolean, initial: number) =>
    h(
      "select",
      { name },
      ...(withNone ? [h("option", { value: "-1" }, "(none)")] : []),
      ...headers.map((t, i) => h("option", { value: String(i), selected: i === initial }, t)),
    );
  const rows = select("rows", false, 0);
  const cols = select("cols", true, -1);
  const vals = select("values", false, Math.min(headers.length - 1, 1));
  const fn = h("select", { name: "fn" }, ...["SUM", "AVERAGE", "COUNT", "COUNTA", "MAX", "MIN"].map((f) => h("option", { value: f }, f)));
  const where = h("select", { name: "where" }, h("option", { value: "new" }, "New sheet"), h("option", { value: "here" }, "Right of the data"));
  const dlg = h(
    "dialog",
    { class: "dialog pivot-dialog", "aria-label": "PivotTable" },
    h(
      "form",
      { method: "dialog" },
      h("h2", {}, "PivotTable"),
      h("p", { class: "muted" }, `Source: ${info.name}, ${headers.length} columns × ${data.r1 - data.r0} rows`),
      h("label", {}, "Rows ", rows),
      h("label", {}, "Columns ", cols),
      h("label", {}, "Values ", vals),
      h("label", {}, "Summarize by ", fn),
      h("label", {}, "Place in ", where),
      h("menu", {}, h("button", { value: "cancel" }, "Cancel"), h("button", { value: "ok", class: "primary" }, "Insert")),
    ),
  );
  dlg.addEventListener("close", () => {
    if (dlg.returnValue === "ok") {
      const colIdx = Number(cols.value);
      const toNew = where.value === "new";
      const formula = pivotFormula({ sheet: info.name, data, rows: Number(rows.value), cols: colIdx < 0 ? null : colIdx, values: Number(vals.value), fn: fn.value as Aggregate, qualify: toNew });
      if (toNew) {
        ed.execute("addSheet", `Pivot ${headers[Number(vals.value)] ?? ""}`.trim());
        ed.applyEdits([{ sheet: ed.sheet, r: 0, c: 0, text: formula }]);
      } else {
        ed.applyEdits([{ sheet: ed.sheet, r: data.r0, c: data.c1 + 2, text: formula }]);
        ed.selectCell(data.r0, data.c1 + 2);
      }
    }
    dlg.remove();
    ed.focus();
  });
  document.body.append(dlg);
  if (typeof dlg.showModal === "function") dlg.showModal();
  else dlg.setAttribute("open", "");
  return dlg;
}
