import { FluentBundle, FluentResource } from "https://cdn.jsdelivr.net/npm/@fluent/bundle/+esm";

const supportedLocales = ["en-CA", "zh-Hans"];
const fallbackLocale = "en-CA";
const selector = document.getElementById("lang-selector");

let currentBundle = null;

function detectInitialLocale() {
    const stored = localStorage.getItem("paiagram-lang");
    if (stored && supportedLocales.includes(stored)) {
        return stored;
    }

    const matched = navigator.languages.find((locale) => supportedLocales.includes(locale));
    return matched ?? fallbackLocale;
}

async function loadBundle(locale) {
    const response = await fetch(`./locales/${locale}.ftl`);
    if (!response.ok) {
        if (locale !== fallbackLocale) {
            return loadBundle(fallbackLocale);
        }
        throw new Error(`Failed to load locale file for ${locale}`);
    }

    const source = await response.text();
    const resource = new FluentResource(source);
    const bundle = new FluentBundle(locale);
    bundle.addResource(resource);
    return bundle;
}

function translateElement(element, bundle) {
    const id = element.dataset.l10nId;
    if (!id) {
        return;
    }

    const message = bundle.getMessage(id);
    if (!message?.value) {
        return;
    }

    const translated = bundle.formatPattern(message.value, undefined, []);

    if (element.tagName === "SELECT") {
        element.setAttribute("aria-label", translated);
        return;
    }

    element.textContent = translated;
}

function translatePage(bundle, locale) {
    currentBundle = bundle;
    document.documentElement.lang = locale;

    document.querySelectorAll("[data-l10n-id]").forEach((element) => {
        translateElement(element, bundle);
    });
}

async function setLocale(locale) {
    const bundle = await loadBundle(locale);
    translatePage(bundle, locale);

    if (selector) {
        selector.value = locale;
    }

    localStorage.setItem("paiagram-lang", locale);
}

async function initLocalization() {
    const locale = detectInitialLocale();
    await setLocale(locale);

    if (selector) {
        selector.addEventListener("change", async (event) => {
            const selected = event.target.value;
            if (!supportedLocales.includes(selected) || !currentBundle) {
                return;
            }

            await setLocale(selected);
        });
    }
}

initLocalization().catch((error) => {
    console.error("Localization initialization failed", error);
});
