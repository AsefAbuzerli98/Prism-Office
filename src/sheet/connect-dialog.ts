/** "From SQL" dialog: SQLite file or ODBC connection string + query. */
import { h } from "../core/dom";

export interface SqlRequest {
  kind: "sqlite" | "odbc";
  source: string;
  query: string;
}

export function openSqlDialog(onSubmit: (req: SqlRequest) => void): HTMLDialogElement {
  const kind = h("select", { name: "kind" }, h("option", { value: "sqlite" }, "SQLite file"), h("option", { value: "odbc" }, "ODBC connection string (needs pyodbc)"));
  const source = h("input", { name: "source", placeholder: "/path/to/data.db  or  DSN=Sales;UID=…", required: true, size: 48 });
  const query = h("textarea", { name: "query", rows: 5, cols: 48, required: true, spellcheck: "false" }, "SELECT * FROM sqlite_master");
  const dlg = h(
    "dialog",
    { class: "dialog sql-dialog", "aria-label": "Get data from SQL" },
    h(
      "form",
      { method: "dialog" },
      h("h2", {}, "Get data from SQL"),
      h("p", { class: "muted" }, "Queries run read-only. Results go to a new sheet; Refresh All re-runs them."),
      h("label", {}, "Type ", kind),
      h("label", {}, "Source ", source),
      h("label", {}, "Query ", query),
      h("menu", {}, h("button", { value: "cancel", formnovalidate: true }, "Cancel"), h("button", { value: "ok", class: "primary" }, "Load")),
    ),
  );
  dlg.addEventListener("close", () => {
    if (dlg.returnValue === "ok" && source.value.trim() && query.value.trim()) onSubmit({ kind: kind.value as SqlRequest["kind"], source: source.value.trim(), query: query.value.trim() });
    dlg.remove();
  });
  document.body.append(dlg);
  if (typeof dlg.showModal === "function") dlg.showModal();
  else dlg.setAttribute("open", "");
  return dlg;
}
