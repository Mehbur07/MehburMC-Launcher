// Telefon mu masaüstü mü? Android WebView'in kimlik metninde "Android" geçer.
// Uygulama başlarken bir kez okunur; arayüz düzeni (alt menü, başlık çubuğu)
// buna göre seçilir (K103).
export const IS_MOBILE =
  typeof navigator !== "undefined" && /Android|iPhone|iPad|iPod/i.test(navigator.userAgent);

/** `<html data-mobile>`: CSS'in telefona özel kuralları bu işarete bakar. */
export function markPlatform(root: HTMLElement = document.documentElement) {
  if (IS_MOBILE) root.dataset.mobile = "";
}
