/**
 * Presentation model — mirrors `prism_slide::PresentationModel` and the
 * `prism_drawingml` types it is built from (serde JSON). Lengths are EMU
 * (914400 per inch), angles 60000ths of a degree, sizes 100ths of a point,
 * percentages 1000ths of a percent.
 */
import blank from "./blank.json";

export type ColorBase =
  | { t: "srgb"; v: string }
  | { t: "scheme"; v: string }
  | { t: "sys"; v: string; last?: string }
  | { t: "preset"; v: string }
  | { t: "hsl"; h: number; s: number; l: number }
  | { t: "scRgb"; r: number; g: number; b: number };
export interface ColorMod {
  name: string;
  val: number;
}
export interface Color {
  base: ColorBase;
  mods?: ColorMod[];
}

export interface Theme {
  name: string;
  colors: [string, string][];
  majorFont: string;
  minorFont: string;
  fillStyles?: string[];
  lineStyles?: string[];
  bgFillStyles?: string[];
}

export interface GradientStop {
  pos: number;
  color: Color;
}
export type Fill =
  | { t: "none" }
  | { t: "solid"; color: Color }
  | { t: "gradient"; stops: GradientStop[]; angle?: number; path?: string; rotWithShape?: boolean }
  /** `relId` is a part name (`/ppt/media/image1.png`) or `url:...`. */
  | { t: "image"; relId: string; crop?: [number, number, number, number]; tile?: boolean }
  | { t: "pattern"; prst: string; fg: Color; bg: Color }
  | { t: "group" };

export interface LineEnd {
  type: string;
  w?: string;
  len?: string;
}
export interface Line {
  width?: number;
  fill?: Fill;
  dash?: string;
  cap?: string;
  join?: string;
  head?: LineEnd;
  tail?: LineEnd;
}
export interface Shadow {
  inner: boolean;
  blur: number;
  dist: number;
  dir: number;
  color: Color;
}
export interface Effects {
  shadow?: Shadow;
  glow?: [number, Color];
  softEdge?: number;
  reflection?: boolean;
}

export interface RunProps {
  size?: number;
  bold?: boolean;
  italic?: boolean;
  underline?: string;
  strike?: string;
  font?: string;
  fill?: Fill;
  baseline?: number;
  cap?: string;
  spacing?: number;
  highlight?: Color;
  link?: string;
  lang?: string;
}
export type Spacing = { t: "pct"; v: number } | { t: "pts"; v: number };
export type Bullet =
  | { t: "none" }
  | { t: "char"; ch: string; font?: string; color?: Color; size?: number }
  | { t: "autoNum"; scheme: string; start?: number };
export interface ParaProps {
  level?: number;
  align?: string;
  marginLeft?: number;
  indent?: number;
  lineSpacing?: Spacing;
  spaceBefore?: Spacing;
  spaceAfter?: Spacing;
  bullet?: Bullet;
  rtl?: boolean;
  /** Default run properties (`a:defRPr`). */
  run?: RunProps;
}
export type Run =
  | { t: "text"; text: string; props?: RunProps }
  | { t: "break"; props?: RunProps }
  | { t: "field"; kind: string; id: string; text: string; props?: RunProps };
export interface Paragraph {
  props?: ParaProps;
  runs?: Run[];
  end?: RunProps;
}
export type Autofit = { t: "none" } | { t: "normal"; fontScale?: number; lineReduction?: number } | { t: "shape" };
export interface BodyProps {
  wrap?: string;
  anchor?: string;
  anchorCtr?: boolean;
  insets?: [number | null, number | null, number | null, number | null];
  autofit?: Autofit;
  vert?: string;
  rot?: number;
  columns?: number;
}
export interface ListStyle {
  default?: ParaProps;
  levels?: (ParaProps | null)[];
}
export interface TextBody {
  body?: BodyProps;
  listStyle?: ListStyle;
  paragraphs?: Paragraph[];
}

export interface Guide {
  name: string;
  op: string;
  args: string[];
}
export type PathFill = "norm" | "none" | "lighten" | "lightenLess" | "darken" | "darkenLess";
export type PathCmd =
  | { c: "M"; x: string; y: string }
  | { c: "L"; x: string; y: string }
  | { c: "A"; wr: string; hr: string; st: string; sw: string }
  | { c: "C"; pts: [string, string, string, string, string, string] }
  | { c: "Q"; pts: [string, string, string, string] }
  | { c: "Z" };
export interface PathDef {
  w: number;
  h: number;
  fill: PathFill;
  stroke: boolean;
  cmds: PathCmd[];
}
export interface GeomDef {
  av: Guide[];
  gd: Guide[];
  paths: PathDef[];
  text?: [string, string, string, string];
}
export type Geometry = { t: "preset"; name: string; adj?: [string, number][] } | { t: "custom"; def: GeomDef };

/** Evaluated geometry (from the drawingml WASM) in shape-local EMU. */
export type Cmd =
  | { c: "M"; x: number; y: number }
  | { c: "L"; x: number; y: number }
  | { c: "C"; x1: number; y1: number; x2: number; y2: number; x: number; y: number }
  | { c: "Q"; x1: number; y1: number; x: number; y: number }
  | { c: "Z" };
export interface EvaluatedGeometry {
  paths: { fill: PathFill; stroke: boolean; cmds: Cmd[] }[];
  textRect: [number, number, number, number];
}

export interface Xfrm {
  x: number;
  y: number;
  cx: number;
  cy: number;
  rot?: number;
  flipH?: boolean;
  flipV?: boolean;
  /** Group child space: chOff x, y, chExt cx, cy. */
  child?: [number, number, number, number];
}
export interface Placeholder {
  kind?: string;
  idx?: number;
  size?: string;
  orient?: string;
}
export interface StyleRef {
  idx: number;
  color?: Color;
}
export interface ShapeStyle {
  line?: StyleRef;
  fill?: StyleRef;
  effect?: StyleRef;
  font?: [string, Color | null];
}
export interface Common {
  id: number;
  name: string;
  descr?: string;
  hidden?: boolean;
  ph?: Placeholder;
  xfrm?: Xfrm;
}
export interface ShapeBody extends Common {
  textBox?: boolean;
  geom?: Geometry;
  fill?: Fill;
  line?: Line;
  effects?: Effects;
  style?: ShapeStyle;
  text?: TextBody;
  connect?: [[number, number] | null, [number, number] | null];
  /** Prism-only data kept in an extLst (round-trips through PowerPoint). */
  prism?: unknown;
}
export interface PictureBody extends Common {
  image: Fill;
  geom?: Geometry;
  line?: Line;
  effects?: Effects;
  media?: string;
}
export interface TableCell {
  text: TextBody;
  fill?: Fill;
  gridSpan?: number;
  rowSpan?: number;
  hMerge?: boolean;
  vMerge?: boolean;
  borders?: [Line | null, Line | null, Line | null, Line | null];
}
export interface Table {
  cols: number[];
  rows: [number, TableCell[]][];
  styleId?: string;
  firstRow?: boolean;
  bandRow?: boolean;
}
export type SlideNode =
  | ({ type: "shape" } & ShapeBody)
  | ({ type: "connector" } & ShapeBody)
  | ({ type: "picture" } & PictureBody)
  | ({ type: "group"; children: SlideNode[] } & Common)
  | ({ type: "frame"; kind: string; table?: Table; relId?: string } & Common)
  | ({ type: "other"; tag: string } & Common);

export interface Background {
  fill?: Fill;
  styleRef?: [number, Color];
}
export interface TextStyles {
  title: ListStyle;
  body: ListStyle;
  other: ListStyle;
}
export interface Layout {
  part: string;
  name: string;
  kind?: string;
  background?: Background;
  shapes: SlideNode[];
  showMasterShapes?: boolean;
}
export interface Master {
  part: string;
  theme: Theme;
  clrMap: [string, string][];
  background?: Background;
  shapes: SlideNode[];
  textStyles: TextStyles;
  layouts: Layout[];
}

export interface Transition {
  kind: string;
  dir?: string;
  option?: string;
  /** Milliseconds. */
  duration?: number;
  advanceClick?: boolean;
  advanceAfter?: number;
}
export type Trigger = "onClick" | "withPrev" | "afterPrev";
export interface Effect {
  shape: number;
  class: "entr" | "exit" | "emph" | "path" | string;
  preset: string;
  presetId: number;
  subtype?: number;
  trigger: Trigger | string;
  delay?: number;
  duration: number;
  path?: string;
  repeat?: number;
}
export interface Timing {
  effects: Effect[];
  /** The file had timing constructs we could not model (kept verbatim). */
  partial?: boolean;
}

export interface Slide {
  /** Package part; absent for slides created in the editor until saved. */
  part?: string;
  layout: string;
  shapes: SlideNode[];
  background?: Background;
  notes: string;
  hidden?: boolean;
  transition?: Transition;
  timing?: Timing;
}

export interface PresentationModel {
  width: number;
  height: number;
  masters: Master[];
  slides: Slide[];
  defaultTextStyle?: ListStyle;
}

export const EMU_PER_PX = 9525;

/** The blank template (same as the host's `templates::blank`). */
export function blankPresentation(): PresentationModel {
  return structuredClone(blank) as unknown as PresentationModel;
}
