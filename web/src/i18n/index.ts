import { createI18n } from "vue-i18n";
import en from "./locales/en";
import zhCN from "./locales/zh-CN";

export type SupportedLocale = "en" | "zh-CN";

export const supportedLocales: SupportedLocale[] = ["en", "zh-CN"];

function detectLocale(): SupportedLocale {
  const saved = localStorage.getItem("locale");
  if (saved && supportedLocales.includes(saved as SupportedLocale)) {
    return saved as SupportedLocale;
  }
  const browserLang = navigator.language;
  if (browserLang.startsWith("zh")) return "zh-CN";
  return "en";
}

const i18n = createI18n({
  legacy: false,
  locale: detectLocale(),
  fallbackLocale: "en",
  messages: {
    en,
    "zh-CN": zhCN,
  },
});

export default i18n;
