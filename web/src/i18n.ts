import { createInstance } from "i18next";
import zhCN from "./locales/zh-CN.json";
import zhTW from "./locales/zh-TW.json";
import en from "./locales/en.json";
import ja from "./locales/ja.json";

export const languages = { "zh-CN": "简体中文", "zh-TW": "繁體中文", en: "English", ja: "日本語" } as const;
export type Language = keyof typeof languages;
export type Key = keyof typeof zhCN;
export const resources = { "zh-CN": zhCN, "zh-TW": zhTW, en, ja };
export const storageKey = "ibasso-language";
export function matchLanguage(value: string): Language | undefined {
    const tag = value.toLowerCase().replaceAll("_", "-");
    if (tag === "zh" || tag.startsWith("zh-")) {
        if (tag.includes("hans")) return "zh-CN";
        if (tag.includes("hant") || /^zh-(tw|hk|mo)(-|$)/.test(tag)) return "zh-TW";
        return "zh-CN";
    }
    if (/^en(-|$)/.test(tag)) return "en";
    if (/^ja(-|$)/.test(tag)) return "ja";
}
function initialLanguage(): Language {
    try {
        const saved = localStorage.getItem(storageKey);
        if (saved && Object.hasOwn(languages, saved)) return saved as Language;
    } catch {
        /* Storage may be disabled. */
    }
    const preferred = typeof navigator === "undefined" ? [] : (navigator.languages ?? [navigator.language]);
    for (const language of preferred) {
        const match = language && matchLanguage(language);
        if (match) return match;
    }
    return "zh-CN";
}
export const i18n = createInstance();
await i18n.init({
    lng: initialLanguage(),
    fallbackLng: "zh-CN",
    supportedLngs: Object.keys(languages),
    load: "currentOnly",
    keySeparator: false,
    nsSeparator: false,
    resources: Object.fromEntries(
        Object.entries(resources).map(([language, translation]) => [language, { translation }]),
    ),
    // All translated content is assigned to text nodes/attributes, never innerHTML.
    interpolation: { escapeValue: false },
});
export function t(key: Key, values: Record<string, unknown> = {}): string {
    return String(i18n.t(key, values));
}
export async function setLanguage(language: Language): Promise<void> {
    if (!Object.hasOwn(languages, language)) return;
    await i18n.changeLanguage(language);
    try {
        localStorage.setItem(storageKey, language);
    } catch {
        /* Session-only selection. */
    }
}
export class LocalizedError extends Error {
    constructor(
        public key: Key,
        public values: Record<string, unknown> = {},
    ) {
        super(t(key, values));
        Object.defineProperty(this, "message", {
            get: () =>
                t(
                    key,
                    Object.fromEntries(Object.entries(values).map(([k, v]) => [k, v instanceof Error ? v.message : v])),
                ),
        });
    }
}
