import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import type { Language } from "../lib/ipc/bindings/Language";
import en from "./en.json";
import tr from "./tr.json";

export type Locale = "tr" | "en";

/** Resolves the "system" setting from the OS/WebView language. */
export function resolveLocale(language: Language, systemLang = navigator.language): Locale {
  if (language === "tr" || language === "en") return language;
  return systemLang.toLowerCase().startsWith("tr") ? "tr" : "en";
}

void i18n.use(initReactI18next).init({
  resources: { tr: { translation: tr }, en: { translation: en } },
  lng: resolveLocale("system"),
  fallbackLng: "en",
  interpolation: { escapeValue: false },
});

export function applyLanguage(language: Language) {
  const locale = resolveLocale(language);
  document.documentElement.lang = locale;
  if (i18n.language !== locale) void i18n.changeLanguage(locale);
}

export default i18n;
