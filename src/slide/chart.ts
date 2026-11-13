/**
 * Charts on slides: the cached values of a DrawingML chart part
 * (c:chartSpace) are read into the spreadsheet chart model and drawn with
 * the same renderer as workbook charts.
 */
import type { ChartData } from "../sheet/engine";
import type { ChartKind } from "../sheet/model";

export interface SlideChart {
  kind: ChartKind;
  title: string;
  data: ChartData;
}

const C = "http://schemas.openxmlformats.org/drawingml/2006/chart";
const A = "http://schemas.openxmlformats.org/drawingml/2006/main";

function kids(el: Element, local: string): Element[] {
  return [...el.children].filter((c) => c.localName === local);
}

function first(el: Element | null | undefined, ...path: string[]): Element | null {
  let cur: Element | null = el ?? null;
  for (const p of path) {
    if (!cur) return null;
    cur = kids(cur, p)[0] ?? null;
  }
  return cur;
}

/** Cached points of a str/num reference or literal (`c:cat`, `c:val`, `c:tx` …). */
function points(el: Element | null): string[] {
  if (!el) return [];
  const cache = el.getElementsByTagNameNS(C, "strCache")[0] ?? el.getElementsByTagNameNS(C, "numCache")[0] ?? el.getElementsByTagNameNS(C, "strLit")[0] ?? el.getElementsByTagNameNS(C, "numLit")[0] ?? null;
  if (!cache) {
    const v = el.getElementsByTagNameNS(C, "v")[0];
    return v ? [v.textContent ?? ""] : [];
  }
  const n = Number(first(cache, "ptCount")?.getAttribute("val") ?? 0);
  const out: string[] = Array.from({ length: n }, () => "");
  for (const pt of kids(cache, "pt")) {
    const i = Number(pt.getAttribute("idx") ?? 0);
    out[i] = first(pt, "v")?.textContent ?? "";
  }
  return out;
}

export function parseChartXml(xml: string): SlideChart | null {
  if (typeof DOMParser === "undefined") return null;
  const doc = new DOMParser().parseFromString(xml, "application/xml");
  const chart = doc.getElementsByTagNameNS(C, "chart")[0];
  const plot = chart ? first(chart, "plotArea") : null;
  if (!chart || !plot) return null;
  const types: [string, ChartKind][] = [
    ["barChart", "column"],
    ["bar3DChart", "column"],
    ["lineChart", "line"],
    ["line3DChart", "line"],
    ["areaChart", "area"],
    ["area3DChart", "area"],
    ["pieChart", "pie"],
    ["pie3DChart", "pie"],
    ["doughnutChart", "pie"],
    ["scatterChart", "scatter"],
    ["bubbleChart", "scatter"],
  ];
  let kind: ChartKind = "column";
  let group: Element | null = null;
  for (const [tag, k] of types) {
    group = first(plot, tag);
    if (group) {
      kind = k;
      if (tag.startsWith("bar") && first(group, "barDir")?.getAttribute("val") === "bar") kind = "bar";
      break;
    }
  }
  if (!group) return null;
  const series = kids(group, "ser").map((s) => {
    const name = points(first(s, "tx"))[0] ?? "";
    const vals = points(first(s, "val") ?? first(s, "yVal"));
    return { name, cats: points(first(s, "cat") ?? first(s, "xVal")), values: vals.map((v) => (v === "" ? null : Number(v))) };
  });
  const categories = series[0]?.cats.length ? series[0].cats : (series[0]?.values.map((_, i) => String(i + 1)) ?? []);
  let title = "";
  const t = first(chart, "title");
  if (t) title = [...t.getElementsByTagNameNS(A, "t")].map((x) => x.textContent ?? "").join("") || (points(first(t, "tx")).join("") ?? "");
  else if (first(chart, "autoTitleDeleted")?.getAttribute("val") !== "1" && series.length === 1) title = series[0]!.name;
  return { kind, title, data: { categories, series: series.map((s) => ({ name: s.name, values: s.values })) } };
}

/** Decode base64 (part bytes from the host) as UTF-8 XML. */
export function decodeXml(b64: string): string {
  const bin = atob(b64);
  const bytes = Uint8Array.from(bin, (c) => c.charCodeAt(0));
  return new TextDecoder().decode(bytes);
}
