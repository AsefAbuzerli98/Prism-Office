/**
 * Minimal i18n: flat key → message dictionaries with `{name}` placeholders,
 * locale negotiation and text direction (RTL readiness).
 */
export type Messages = Record<string, string>;

const en: Messages = {
  "app.name": "Prism Office",
  "hub.subtitle": "Choose a workspace to begin.",
  "hub.open": "Open",
  "hub.openFile": "Open file…",
  "hub.pathPlaceholder": "Path to .docx, .xlsx or .pptx",
  "hub.recent": "Recent",
  "hub.noRecent": "No recent files yet.",
  "hub.clearRecent": "Clear",
  "hub.missing": "Missing",
  "hub.settings": "Settings",
  "hub.new": "New {mode}",
  "hub.modality": "{app} modality",
  "mode.document": "Document",
  "mode.workbook": "Workbook",
  "mode.presentation": "Presentation",
  "mode.document.tagline": "Write, structure and cite with an AI drafting agent.",
  "mode.workbook.tagline": "Dynamic arrays, regex functions and in-cell Python.",
  "mode.presentation.tagline": "Vector slides, Cameo video and a recording studio.",
  "shell.hub": "Application Hub",
  "shell.newTab": "New tab",
  "shell.closeTab": "Close {title}",
  "shell.detach": "Open in new window",
  "shell.openDocs": "{n} open",
  "shell.notSaved": "Not saved",
  "shell.native": "Native host",
  "shell.browser": "Browser preview",
  "shell.saved": "Saved {title}",
  "shell.issues": "Opened with {n} validation issue(s)",
  "shell.confirmClose": "Close “{title}” without saving?",
  "shell.phase": "{label} — arrives in Phase {phase}",
  "shell.services": "{running} of {total} services running",
  "word.styles": "Paragraph styles",
  "word.font": "Font",
  "word.size": "Size",
  "word.margins": "Margins",
  "word.margins.normal": "Normal",
  "word.margins.narrow": "Narrow",
  "word.margins.moderate": "Moderate",
  "word.margins.wide": "Wide",
  "word.portrait": "Portrait",
  "word.landscape": "Landscape",
  "word.loading": "Loading document…",
  "palette.placeholder": "Type a command…",
  "palette.empty": "No matching commands",
  "settings.title": "Settings",
  "settings.windowing": "Windowing",
  "settings.windowing.tabbed": "Tabbed (one window, many tabs)",
  "settings.windowing.multiInstance": "Multi-instance (one window per document)",
  "settings.windowing.browserNote": "Multi-instance windows require the desktop app.",
  "settings.appearance": "Appearance",
  "settings.appearance.system": "System",
  "settings.appearance.light": "Light",
  "settings.appearance.dark": "Dark",
  "settings.language": "Language",
  "settings.language.auto": "Follow system",
  "settings.services": "Background services",
  "settings.close": "Close",
  "cmd.file.new": "New…",
  "cmd.file.open": "Open…",
  "cmd.file.save": "Save",
  "cmd.file.saveAs": "Save As…",
  "cmd.file.close": "Close Tab",
  "cmd.view.hub": "Show Application Hub",
  "cmd.view.nextTab": "Next Tab",
  "cmd.view.prevTab": "Previous Tab",
  "cmd.window.detach": "Move Tab to New Window",
  "cmd.app.palette": "Command Palette",
  "cmd.app.settings": "Settings",
  "cmd.new.document": "New Document",
  "cmd.new.workbook": "New Workbook",
  "cmd.new.presentation": "New Presentation",
};

const az: Messages = {
  "app.name": "Prism Office",
  "hub.subtitle": "Başlamaq üçün iş sahəsi seçin.",
  "hub.open": "Aç",
  "hub.openFile": "Fayl aç…",
  "hub.pathPlaceholder": ".docx, .xlsx və ya .pptx faylının yolu",
  "hub.recent": "Son fayllar",
  "hub.noRecent": "Hələ son fayl yoxdur.",
  "hub.clearRecent": "Təmizlə",
  "hub.missing": "Tapılmadı",
  "hub.settings": "Ayarlar",
  "hub.new": "Yeni {mode}",
  "hub.modality": "{app} rejimi",
  "mode.document": "Sənəd",
  "mode.workbook": "Cədvəl",
  "mode.presentation": "Təqdimat",
  "mode.document.tagline": "AI qaralama agenti ilə yazın, strukturlaşdırın və istinad edin.",
  "mode.workbook.tagline": "Dinamik massivlər, regex funksiyaları və xanada Python.",
  "mode.presentation.tagline": "Vektor slaydlar, Cameo video və səsyazma studiyası.",
  "shell.hub": "Tətbiq Mərkəzi",
  "shell.newTab": "Yeni tab",
  "shell.closeTab": "{title} bağla",
  "shell.detach": "Yeni pəncərədə aç",
  "shell.openDocs": "{n} açıq",
  "shell.notSaved": "Saxlanmayıb",
  "shell.native": "Native host",
  "shell.browser": "Brauzer önizləməsi",
  "shell.saved": "{title} saxlanıldı",
  "shell.issues": "{n} yoxlama xətası ilə açıldı",
  "shell.confirmClose": "“{title}” saxlanmadan bağlansın?",
  "shell.phase": "{label} — Faza {phase}-də gələcək",
  "shell.services": "{total} servisdən {running} işləyir",
  "word.styles": "Abzas üslubları",
  "word.font": "Şrift",
  "word.size": "Ölçü",
  "word.margins": "Kənar boşluqlar",
  "word.margins.normal": "Normal",
  "word.margins.narrow": "Dar",
  "word.margins.moderate": "Orta",
  "word.margins.wide": "Geniş",
  "word.portrait": "Portret",
  "word.landscape": "Albom",
  "word.loading": "Sənəd yüklənir…",
  "palette.placeholder": "Əmr yazın…",
  "palette.empty": "Uyğun əmr yoxdur",
  "settings.title": "Ayarlar",
  "settings.windowing": "Pəncərə rejimi",
  "settings.windowing.tabbed": "Tablı (bir pəncərə, çox tab)",
  "settings.windowing.multiInstance": "Çox pəncərəli (hər sənəd ayrı pəncərədə)",
  "settings.windowing.browserNote": "Çox pəncərəli rejim yalnız desktop tətbiqində işləyir.",
  "settings.appearance": "Görünüş",
  "settings.appearance.system": "Sistem",
  "settings.appearance.light": "Açıq",
  "settings.appearance.dark": "Tünd",
  "settings.language": "Dil",
  "settings.language.auto": "Sistemə uyğun",
  "settings.services": "Fon servisləri",
  "settings.close": "Bağla",
  "cmd.file.new": "Yeni…",
  "cmd.file.open": "Aç…",
  "cmd.file.save": "Saxla",
  "cmd.file.saveAs": "Fərqli saxla…",
  "cmd.file.close": "Tabı bağla",
  "cmd.view.hub": "Tətbiq Mərkəzini göstər",
  "cmd.view.nextTab": "Növbəti tab",
  "cmd.view.prevTab": "Əvvəlki tab",
  "cmd.window.detach": "Tabı yeni pəncərəyə köçür",
  "cmd.app.palette": "Əmr paneli",
  "cmd.app.settings": "Ayarlar",
  "cmd.new.document": "Yeni sənəd",
  "cmd.new.workbook": "Yeni cədvəl",
  "cmd.new.presentation": "Yeni təqdimat",
  // ribbon
  "ribbon.tab.file": "Fayl",
  "ribbon.tab.typography": "Tipoqrafiya",
  "ribbon.tab.page-setup": "Səhifə",
  "ribbon.tab.citations": "İstinadlar",
  "ribbon.tab.structure": "Sənəd strukturu",
  "ribbon.tab.data": "Məlumat bağlantıları",
  "ribbon.tab.formulas": "Düsturlar",
  "ribbon.tab.python": "Python",
  "ribbon.tab.pivot": "Pivot cədvəllər",
  "ribbon.tab.design": "Slayd dizaynı",
  "ribbon.tab.cameo": "Cameo",
  "ribbon.tab.transitions": "Keçidlər",
  "ribbon.tab.recording": "Səsyazma studiyası",
  "ribbon.group.file": "Fayl",
  "ribbon.group.font": "Şrift",
  "ribbon.group.paragraph": "Abzas",
  "ribbon.group.styles": "Üslublar",
  "ribbon.group.page": "Səhifə",
  "ribbon.group.sources": "Mənbələr",
  "ribbon.group.structure": "Struktur",
  "ribbon.group.ai": "AI",
  "ribbon.group.get-data": "Məlumat al",
  "ribbon.group.library": "Funksiyalar",
  "ribbon.group.py": "Xanada Python",
  "ribbon.group.pivot": "Pivot",
  "ribbon.group.slides": "Slaydlar",
  "ribbon.group.cameo": "Cameo",
  "ribbon.group.transitions": "Keçidlər",
  "ribbon.group.record": "Yazı",
  "ribbon.cmd.file.new": "Yeni",
  "ribbon.cmd.file.open": "Aç",
  "ribbon.cmd.file.save": "Saxla",
  "ribbon.cmd.file.saveAs": "Fərqli saxla",
  "ribbon.cmd.doc.bold": "Qalın",
  "ribbon.cmd.doc.italic": "Kursiv",
  "ribbon.cmd.doc.underline": "Altdan xətt",
  "ribbon.cmd.doc.font": "Şrift",
  "ribbon.cmd.doc.strike": "Üstündən xətt",
  "ribbon.cmd.doc.pageBreak": "Səhifə sonu",
  "ribbon.cmd.doc.bullets": "Markerlər",
  "ribbon.cmd.doc.numbering": "Nömrələmə",
  "ribbon.cmd.doc.align": "Düzləndirmə",
  "ribbon.cmd.doc.styles": "Üslublar",
  "ribbon.cmd.doc.margins": "Kənar boşluqlar",
  "ribbon.cmd.doc.orientation": "İstiqamət",
  "ribbon.cmd.doc.columns": "Sütunlar",
  "ribbon.cmd.doc.footnote": "Haşiyə",
  "ribbon.cmd.doc.citation": "İstinad",
  "ribbon.cmd.doc.bibliography": "Biblioqrafiya",
  "ribbon.cmd.doc.toc": "Mündəricat",
  "ribbon.cmd.doc.markdown": "Markdown görünüşü",
  "ribbon.cmd.doc.accessibility": "Əlçatanlıq",
  "ribbon.cmd.doc.agent": "Agentlə yaz",
  "ribbon.cmd.xl.importCsv": "CSV-dən",
  "ribbon.cmd.xl.importSql": "SQL-dən",
  "ribbon.cmd.xl.refresh": "Hamısını yenilə",
  "ribbon.cmd.xl.groupby": "GROUPBY",
  "ribbon.cmd.xl.pivotby": "PIVOTBY",
  "ribbon.cmd.xl.regex": "REGEX",
  "ribbon.cmd.xl.xlookup": "XLOOKUP",
  "ribbon.cmd.xl.py": "=PY əlavə et",
  "ribbon.cmd.xl.pyEnv": "Mühit",
  "ribbon.cmd.xl.pivotTable": "Pivot cədvəl",
  "ribbon.cmd.xl.chart": "Diaqram",
  "ribbon.cmd.ppt.newSlide": "Yeni slayd",
  "ribbon.cmd.ppt.layout": "Maket",
  "ribbon.cmd.ppt.theme": "Mövzu",
  "ribbon.cmd.ppt.generate": "Təqdimat yarat",
  "ribbon.cmd.ppt.cameo": "Cameo əlavə et",
  "ribbon.cmd.ppt.cameoShape": "Çərçivə forması",
  "ribbon.cmd.ppt.morph": "Morph",
  "ribbon.cmd.ppt.fade": "Solma",
  "ribbon.cmd.ppt.timeline": "Animasiya xətti",
  "ribbon.cmd.ppt.record": "Yaz",
  "ribbon.cmd.ppt.teleprompter": "Teleprompter",
  "ribbon.cmd.ppt.export": "MP4 ixrac",
  "ribbon.cmd.ppt.coach": "Çıxış məşqçisi",
};

export const CATALOGS: Record<string, Messages> = { en, az };
export const SUPPORTED_LOCALES = Object.keys(CATALOGS);
export const LOCALE_NAMES: Record<string, string> = { en: "English", az: "Azərbaycanca" };

const RTL_LANGS = new Set(["ar", "he", "fa", "ur", "ps", "sd", "yi", "dv", "ug"]);

/** Pick the best supported locale for a preference list (BCP-47 tags). */
export function negotiateLocale(preferred: readonly string[], supported = SUPPORTED_LOCALES): string {
  for (const tag of preferred) {
    const lower = tag.toLowerCase();
    if (supported.includes(lower)) return lower;
    const base = lower.split("-")[0]!;
    if (supported.includes(base)) return base;
  }
  return "en";
}

export function directionOf(locale: string): "ltr" | "rtl" {
  return RTL_LANGS.has(locale.toLowerCase().split("-")[0]!) ? "rtl" : "ltr";
}

let current = "en";
const listeners = new Set<() => void>();

export function setLocale(locale: string | null, root: HTMLElement = document.documentElement) {
  current = locale && CATALOGS[locale] ? locale : negotiateLocale(navigator.languages ?? [navigator.language]);
  root.lang = current;
  root.dir = directionOf(current);
  for (const l of listeners) l();
}

export function getLocale() {
  return current;
}

export function onLocaleChange(cb: () => void) {
  listeners.add(cb);
  return () => listeners.delete(cb);
}

/** Translate `key`, falling back to English, then to `fallback`, then the key. */
export function t(key: string, params: Record<string, string | number> = {}, fallback?: string): string {
  const msg = CATALOGS[current]?.[key] ?? en[key] ?? fallback ?? key;
  return msg.replace(/\{(\w+)\}/g, (m, name: string) => (name in params ? String(params[name]) : m));
}
