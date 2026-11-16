import { ipc } from "./ipc";

// Mac mi? macOS'taki WebView'in kimlik metninde "Macintosh" geçer (K107).
// Uygulama başlarken bir kez okunur: başlık çubuğu, kısayol yazıları ve menü
// dili buna göre seçilir.
export const IS_MAC =
  typeof navigator !== "undefined" && /Macintosh|Mac OS X/i.test(navigator.userAgent);

/**
 * Kısayol yazısı: Mac'te "⌘Z" / "⇧⌘Z", başka yerde "Ctrl+Z" / "Ctrl+Shift+Z".
 * `key` tek bir tuş adıdır ("Z", "Y").
 */
export function shortcut(key: string, opts: { shift?: boolean } = {}, mac = IS_MAC): string {
  if (mac) return `${opts.shift ? "⇧" : ""}⌘${key}`;
  return `Ctrl+${opts.shift ? "Shift+" : ""}${key}`;
}

let menuLocale: string | null = null;

/**
 * Mac menü çubuğunu (Düzen, Pencere…) arayüzün diline çevirir. Yalnız dil
 * değişince Rust'a gider; Mac dışında hiçbir şey yapmaz.
 */
export function syncMenuLanguage(locale: string, mac = IS_MAC) {
  if (!mac || menuLocale === locale) return;
  menuLocale = locale;
  ipc.setMenuLanguage(locale).catch(() => {
    menuLocale = null; // bir sonraki ayar değişikliğinde yeniden dener
  });
}
