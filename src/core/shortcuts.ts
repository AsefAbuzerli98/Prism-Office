/**
 * Keyboard shortcut catalogue: the Word, Excel and PowerPoint shortcuts that
 * Prism Office supports, per mode. It feeds the Keyboard Shortcuts dialog
 * (F1 / Mod+/) and the ribbon tooltips ("Bold (Ctrl+B)"). The handlers live in
 * the editors and the shell command registry; this file only describes them.
 */
import { formatKeybinding, isMacPlatform } from "./commands";
import { getLocale } from "./i18n";
import type { Mode } from "./modes";

export interface Shortcut {
  /** One or more keybindings in `parseKeybinding` syntax ("Mod+B", "Shift+F5"). */
  keys: string[];
  en: string;
  az: string;
  /** Ribbon command whose tooltip shows these keys. */
  cmd?: string;
}

export interface ShortcutSection {
  en: string;
  az: string;
  items: Shortcut[];
}

const s = (keys: string | string[], en: string, az: string, cmd?: string): Shortcut => ({
  keys: Array.isArray(keys) ? keys : [keys],
  en,
  az,
  cmd,
});

const GENERAL: ShortcutSection = {
  en: "Everywhere",
  az: "Hər yerdə",
  items: [
    s("Mod+N", "New document (home)", "Yeni sənəd (ana səhifə)", "file.new"),
    s("Mod+O", "Open a file", "Fayl aç", "file.open"),
    s("Mod+S", "Save", "Saxla", "file.save"),
    s(["Mod+Shift+S", "F12"], "Save as", "Fərqli saxla", "file.saveAs"),
    s("Mod+W", "Close the tab", "Tab-ı bağla"),
    s(["Ctrl+Tab", "Ctrl+Shift+Tab"], "Next / previous tab", "Növbəti / əvvəlki tab"),
    s(["Mod+K", "Mod+Shift+P"], "Command palette", "Əmrlər paneli"),
    s("Mod+,", "Settings", "Ayarlar"),
    s(["F1", "Mod+/"], "Keyboard shortcuts", "Klaviatura qısayolları"),
    s(["Mod+Z", "Mod+Y"], "Undo / redo", "Geri al / təkrarla"),
    s(["Mod+X", "Mod+C", "Mod+V"], "Cut / copy / paste", "Kəs / kopyala / yapışdır"),
    s("Mod+A", "Select all", "Hamısını seç"),
  ],
};

const DOCUMENT: ShortcutSection[] = [
  {
    en: "Character formatting",
    az: "Simvol formatı",
    items: [
      s("Mod+B", "Bold", "Qalın", "doc.bold"),
      s("Mod+I", "Italic", "Kursiv", "doc.italic"),
      s("Mod+U", "Underline", "Altdan xətt", "doc.underline"),
      s("Mod+D", "Font", "Şrift", "doc.font"),
      s(["Mod+Shift+>", "Mod+]"], "Larger font", "Şrifti böyüt"),
      s(["Mod+Shift+<", "Mod+["], "Smaller font", "Şrifti kiçilt"),
      s("Mod+=", "Subscript", "Aşağı indeks"),
      s("Mod+Shift+=", "Superscript", "Yuxarı indeks"),
      s("Mod+Space", "Clear character formatting", "Simvol formatını təmizlə"),
      s(["Mod+Shift+C", "Mod+Shift+V"], "Copy / paste formatting", "Formatı kopyala / tətbiq et"),
      s("Shift+F3", "Change case", "Hərf registrini dəyiş"),
    ],
  },
  {
    en: "Paragraphs",
    az: "Abzaslar",
    items: [
      s("Mod+L", "Align left", "Sola düzləndir", "doc.alignLeft"),
      s("Mod+E", "Center", "Mərkəzə düzləndir", "doc.alignCenter"),
      s("Mod+R", "Align right", "Sağa düzləndir", "doc.alignRight"),
      s("Mod+J", "Justify", "Hər iki tərəfə düzləndir", "doc.justify"),
      s("Mod+Shift+L", "Bullets", "Markerli siyahı", "doc.bullets"),
      s("Mod+M", "Increase indent", "Girintini artır", "doc.indent"),
      s("Mod+Shift+M", "Decrease indent", "Girintini azalt", "doc.outdent"),
      s(["Mod+1", "Mod+5", "Mod+2"], "Line spacing 1 / 1.5 / 2", "Sətir aralığı 1 / 1,5 / 2", "doc.lineSpacing"),
      s(["Mod+Alt+1", "Mod+Alt+2", "Mod+Alt+3"], "Heading 1 / 2 / 3", "Başlıq 1 / 2 / 3", "doc.styles"),
      s("Mod+Shift+N", "Normal style", "Normal üslub"),
    ],
  },
  {
    en: "Insert and review",
    az: "Daxil et və yoxla",
    items: [
      s("Mod+Enter", "Page break", "Səhifə keçidi", "doc.pageBreak"),
      s("Shift+Enter", "Line break", "Sətir keçidi"),
      s("Mod+Alt+F", "Footnote", "Haşiyə", "doc.footnote"),
      s("Mod+Alt+D", "Endnote", "Son qeyd", "doc.endnote"),
      s("Mod+Alt+M", "New comment", "Yeni şərh", "doc.comment"),
      s("Mod+Shift+E", "Track changes", "Dəyişiklikləri izlə", "doc.trackChanges"),
      s("F9", "Update fields", "Sahələri yenilə", "doc.updateFields"),
      s("Mod+F", "Find", "Tap"),
      s("Mod+H", "Replace", "Əvəz et"),
      s("Mod+Alt+Shift+M", "Markdown view", "Markdown görünüşü", "doc.markdown"),
    ],
  },
  {
    en: "Moving around",
    az: "Hərəkət",
    items: [
      s(["Mod+ArrowLeft", "Mod+ArrowRight"], "Word left / right", "Söz sola / sağa"),
      s(["Mod+Home", "Mod+End"], "Start / end of document", "Sənədin əvvəli / sonu"),
      s(["Mod+Backspace", "Mod+Delete"], "Delete word", "Sözü sil"),
      s("Tab", "Next table cell", "Növbəti cədvəl xanası"),
      s("Escape", "Leave header, footer or note", "Başlıqdan, altlıqdan və ya qeyddən çıx"),
    ],
  },
];

const WORKBOOK: ShortcutSection[] = [
  {
    en: "Cells and editing",
    az: "Xanalar və redaktə",
    items: [
      s("F2", "Edit the cell", "Xananı redaktə et"),
      s(["Enter", "Shift+Enter"], "Confirm and move down / up", "Təsdiqlə, aşağı / yuxarı keç"),
      s(["Tab", "Shift+Tab"], "Confirm and move right / left", "Təsdiqlə, sağa / sola keç"),
      s("Alt+Enter", "New line in the cell", "Xanada yeni sətir"),
      s("Mod+Enter", "Fill the selection with the entry", "Seçimi daxil edilənlə doldur"),
      s("F4", "Toggle absolute reference ($)", "Mütləq istinad ($)"),
      s("Delete", "Clear contents", "Məzmunu təmizlə"),
      s(["Mod+D", "Mod+R"], "Fill down / right", "Aşağı / sağa doldur"),
      s("Alt+=", "AutoSum", "Avtocəm", "xl.autoSum"),
      s(["Mod+;", "Mod+Shift+;"], "Insert today's date / time", "Bugünkü tarix / vaxt"),
      s("Mod+'", "Copy formula from the cell above", "Yuxarıdakı xananın düsturunu köçür"),
    ],
  },
  {
    en: "Formatting",
    az: "Format",
    items: [
      s("Mod+B", "Bold", "Qalın", "xl.bold"),
      s("Mod+I", "Italic", "Kursiv", "xl.italic"),
      s("Mod+U", "Underline", "Altdan xətt", "xl.underline"),
      s("Mod+5", "Strikethrough", "Üstündən xətt"),
      s("Mod+1", "Format cells", "Xanaları formatla", "xl.numfmt"),
      s("Mod+Shift+~", "General format", "Ümumi format"),
      s("Mod+Shift+!", "Number format (#,##0.00)", "Ədəd formatı (#,##0.00)"),
      s("Mod+Shift+$", "Currency format", "Valyuta formatı"),
      s("Mod+Shift+%", "Percent format", "Faiz formatı", "xl.percent"),
      s("Mod+Shift+#", "Date format", "Tarix formatı"),
      s("Mod+Shift+@", "Time format", "Vaxt formatı"),
      s("Mod+Shift+^", "Scientific format", "Elmi format"),
      s(["Mod+Shift+&", "Mod+Shift+_"], "Add / remove outline border", "Kənar xətti əlavə et / sil", "xl.border"),
    ],
  },
  {
    en: "Selection and sheets",
    az: "Seçim və vərəqlər",
    items: [
      s("Mod+Arrow", "Jump to the edge of the data", "Verilənlərin kənarına keç"),
      s("Mod+Shift+Arrow", "Extend to the edge of the data", "Seçimi verilənlərin kənarına uzat"),
      s(["Mod+Home", "Mod+End"], "First / last used cell", "İlk / son istifadə olunan xana"),
      s("Mod+Space", "Select the column", "Sütunu seç"),
      s("Shift+Space", "Select the row", "Sətri seç"),
      s(["Mod+PageUp", "Mod+PageDown"], "Previous / next sheet", "Əvvəlki / növbəti vərəq"),
      s("Shift+F11", "New sheet", "Yeni vərəq", "xl.addSheet"),
      s("F11", "Insert chart", "Diaqram daxil et", "xl.chart"),
      s("Mod+Alt+F5", "Refresh all connections", "Bütün bağlantıları yenilə", "xl.refresh"),
    ],
  },
];

const PRESENTATION: ShortcutSection[] = [
  {
    en: "Slides",
    az: "Slaydlar",
    items: [
      s("Mod+M", "New slide", "Yeni slayd", "ppt.newSlide"),
      s("Mod+D", "Duplicate", "Dublikat", "ppt.duplicateSlide"),
      s(["Mod+ArrowUp", "Mod+ArrowDown"], "Move slide up / down", "Slaydı yuxarı / aşağı köçür"),
      s(["PageUp", "PageDown"], "Previous / next slide", "Əvvəlki / növbəti slayd"),
      s(["Home", "End"], "First / last slide", "İlk / son slayd"),
      s("Delete", "Delete the shape or slide", "Fiquru və ya slaydı sil", "ppt.deleteSlide"),
    ],
  },
  {
    en: "Text and shapes",
    az: "Mətn və fiqurlar",
    items: [
      s("Mod+B", "Bold", "Qalın", "ppt.bold"),
      s("Mod+I", "Italic", "Kursiv", "ppt.italic"),
      s("Mod+U", "Underline", "Altdan xətt", "ppt.underline"),
      s("Mod+Shift+>", "Grow font", "Şrifti böyüt", "ppt.grow"),
      s("Mod+Shift+<", "Shrink font", "Şrifti kiçilt", "ppt.shrink"),
      s("Mod+L", "Align left", "Sola düzləndir", "ppt.alignLeft"),
      s("Mod+E", "Center", "Mərkəzə düzləndir", "ppt.alignCenter"),
      s("Mod+R", "Align right", "Sağa düzləndir", "ppt.alignRight"),
      s("Mod+J", "Justify", "Hər iki tərəfə düzləndir", "ppt.justify"),
      s(["Mod+G", "Mod+Shift+G"], "Group / ungroup", "Qrupla / qrupu ayır"),
      s(["Mod+]", "Mod+["], "Bring forward / send backward", "Önə / arxaya göndər", "ppt.arrange"),
      s(["Enter", "F2"], "Edit the text of the shape", "Fiqurun mətnini redaktə et"),
      s(["Tab", "Shift+Tab"], "Next / previous shape", "Növbəti / əvvəlki fiqur"),
      s(["Arrow", "Shift+Arrow"], "Nudge (1 pt / 10 pt)", "Sürüşdür (1 pt / 10 pt)"),
    ],
  },
  {
    en: "Slide show",
    az: "Slayd şou",
    items: [
      s("F5", "From the beginning", "Əvvəldən", "ppt.showStart"),
      s("Shift+F5", "From the current slide", "Cari slayddan", "ppt.showCurrent"),
      s("Alt+F5", "Presenter view", "Məruzəçi görünüşü", "ppt.presenter"),
      s(["N", "Space", "ArrowRight"], "Next animation or slide", "Növbəti animasiya və ya slayd"),
      s(["P", "Backspace", "ArrowLeft"], "Previous", "Əvvəlki"),
      s(["B", "W"], "Black / white screen", "Qara / ağ ekran"),
      s("Enter", "Type a number, then Enter: go to that slide", "Rəqəm yazıb Enter: o slayda keç"),
      s("Escape", "End the show", "Şounu bitir"),
    ],
  },
];

export function shortcutSections(mode: Mode | "hub"): ShortcutSection[] {
  const own = mode === "document" ? DOCUMENT : mode === "workbook" ? WORKBOOK : mode === "presentation" ? PRESENTATION : [];
  return [...own, GENERAL];
}

/** Localised label of a catalogue entry. */
export function label(x: { en: string; az: string }): string {
  return getLocale().startsWith("az") ? x.az : x.en;
}

/** "Ctrl+B" / "⌘B", for the ribbon tooltip of a command (first binding). */
export function shortcutFor(cmd: string, mac = isMacPlatform()): string | null {
  for (const sec of [...DOCUMENT, ...WORKBOOK, ...PRESENTATION, GENERAL]) {
    const hit = sec.items.find((i) => i.cmd === cmd);
    if (hit) return formatKeys(hit.keys[0]!, mac);
  }
  return null;
}

/** Display form of one binding; "Arrow" stands for any arrow key. */
export function formatKeys(spec: string, mac = isMacPlatform()): string {
  return formatKeybinding(spec, mac).replace(/Arrow(Left|Right|Up|Down)/, (_m, d: string) => ({ Left: "←", Right: "→", Up: "↑", Down: "↓" })[d]!).replace(/Arrow$/, "←↑→↓");
}
