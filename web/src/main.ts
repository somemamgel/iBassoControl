import { i18n, t, languages, setLanguage, LocalizedError, type Key, type Language } from "./i18n";
import "./style.css";
import { FILTERS, GAINS, OUTPUTS, DEVICE_FILTER, hex, volumeLevel, type Setting, type Status } from "./protocol";
import { WebHidClient, browserPort, supportsDevice, acquireDeviceLease, asError } from "./webhid";

const app = document.querySelector<HTMLDivElement>("#app")!;
app.innerHTML = `
  <header class="topbar">
    <a class="brand" href="./" data-i18n-aria="home"><span class="brand-mark" aria-hidden="true">iB</span><span>iBasso <span class="font-normal text-stone-500">Control</span></span></a>
    <label class="language-picker"><span data-i18n="language"></span><select id="language" data-i18n-aria="language"></select></label>
    <span class="browser-tag"><span class="dot" aria-hidden="true"></span> WEBHID</span>
  </header>
  <main class="workspace">
    <aside class="device-panel">
      <div class="flex items-center justify-between"><span class="eyebrow text-stone-400"><span data-i18n="audio"></span></span><span id="connection-badge" class="connection-badge"><span data-i18n="disconnected"></span></span></div>
      <span class="eyebrow text-stone-400"><span data-i18n="portable"></span></span><h1 class="mt-2 text-3xl font-medium tracking-tight">DC04 Pro<span class="text-lime-300">.</span></h1>
      <dl class="device-meta"><div><dt><span data-i18n="connection"></span></dt><dd>USB · WebHID</dd></div><div><dt><span data-i18n="firmware"></span></dt><dd id="firmware">—</dd></div><div><dt><span data-i18n="deviceId"></span></dt><dd>262A : 187F</dd></div></dl>
      <button id="connect" class="connect-button" type="button"><span><span data-i18n="connect"></span></span><span aria-hidden="true">↗</span></button>
      <button id="disconnect" class="disconnect-button" type="button" disabled><span data-i18n="disconnect"></span></button>
      <div id="notice" class="notice" role="status" aria-live="polite"></div>
    </aside>
    <section id="hub" hidden class="controls-panel" data-i18n-aria="controls">
      <div class="page-heading"><h2 class="text-3xl font-medium"><span data-i18n="controls"></span></h2><button id="refresh" class="secondary-button" type="button" disabled><span aria-hidden="true">↻</span><span data-i18n="refresh"></span></button></div>
      <div class="section-line"><span class="eyebrow"><span data-i18n="settings"></span></span><span id="updated" class="text-xs text-stone-500"><span data-i18n="waiting"></span></span></div>
      <form id="volume-form" class="card volume-card">
        <fieldset id="volume-fields" disabled>
          <div class="flex items-start justify-between gap-4"><div><span class="card-index">01 / <span data-i18n="volumeTitle"></span></span><h3 class="card-title"><span data-i18n="volumeTitle"></span></h3></div><div class="volume-number"><output id="volume-draft" for="volume-range volume-level">—</output><span>/ 100</span></div></div>
          <label class="sr-only" for="volume-range"><span data-i18n="volumeSlider"></span></label><input id="volume-range" type="range" min="0" max="100" value="0" class="volume-range" />
          <div class="flex justify-between text-xs text-stone-400"><span><span data-i18n="mute"></span></span><span><span data-i18n="maxVolume"></span></span></div>
          <div class="volume-controls"><label class="number-label" for="volume-level"><span data-i18n="setVolume"></span><input id="volume-level" type="number" min="0" max="100" step="1" placeholder="0–100" required /></label><p id="volume-current" class="current-value"><span data-i18n="volumeEmpty"></span></p></div>
          <div class="balance-row"><label for="balance"><span data-i18n="balance"></span></label><div><input id="balance" type="range" min="-10" max="10" value="0" /><div class="flex justify-between text-xs text-stone-400 mt-2"><span><span data-i18n="left"></span> -10</span><output id="balance-draft" for="balance"><span data-i18n="center"></span></output><span><span data-i18n="right"></span> +10</span></div></div></div>
          <div class="card-footer"><button class="primary-button" type="submit"><span data-i18n="applyVolume"></span><span aria-hidden="true">↗</span></button></div>
        </fieldset>
      </form>
      <div class="settings-grid">
        <form id="filter-form" class="card"><fieldset disabled><span class="card-index">02 / <span data-i18n="filter"></span></span><h3 class="card-title"><span data-i18n="filter"></span></h3><label class="sr-only" for="filter"><span data-i18n="filter"></span></label><select id="filter" required><option value="" data-i18n="chooseFilter"></option><option value="d1" data-filter-label="d1"></option><option value="d2" data-filter-label="d2"></option><option value="d3" data-filter-label="d3"></option><option value="d4" data-filter-label="d4"></option><option value="d5" data-filter-label="d5"></option></select><div class="small-footer"><p id="filter-current" class="current-value">—</p><button class="small-button" type="submit"><span data-i18n="apply"></span></button></div></fieldset></form>
        <form id="gain-form" class="card"><fieldset disabled><span class="card-index">03 / <span data-i18n="gain"></span></span><h3 class="card-title"><span data-i18n="gain"></span></h3><label class="sr-only" for="gain"><span data-i18n="gain"></span></label><select id="gain" required><option value="" data-i18n="chooseGain"></option><option value="low" data-i18n="low"></option><option value="medium" data-i18n="medium"></option><option value="high" data-i18n="high"></option></select><div class="small-footer"><p id="gain-current" class="current-value">—</p><button class="small-button" type="submit"><span data-i18n="apply"></span></button></div></fieldset></form>
        <form id="output-form" class="card"><fieldset disabled><span class="card-index">04 / <span data-i18n="output"></span></span><h3 class="card-title"><span data-i18n="output"></span></h3><label class="sr-only" for="output"><span data-i18n="output"></span></label><select id="output" required><option value="" data-i18n="chooseOutput"></option><option value="normal" data-i18n="normal"></option><option value="turbo" data-i18n="turbo"></option></select><div class="small-footer"><p id="output-current" class="current-value">—</p><button class="small-button" type="submit"><span data-i18n="apply"></span></button></div></fieldset></form>
        <form id="eco-form" class="card"><fieldset disabled><span class="card-index">05 / <span data-i18n="eco"></span></span><h3 class="card-title"><span data-i18n="eco"></span></h3><label class="sr-only" for="eco"><span data-i18n="eco"></span></label><select id="eco" required><option value="" data-i18n="chooseEco"></option><option value="off" data-i18n="off"></option><option value="on" data-i18n="on"></option></select><div class="small-footer"><p id="eco-current" class="current-value">—</p><button class="small-button" type="submit"><span data-i18n="apply"></span></button></div></fieldset></form>
      </div>
      <details class="diagnostics"><summary><span><span data-i18n="diagnostics"></span></span><span class="text-xs font-normal text-stone-500"><span data-i18n="environment"></span></span></summary><div class="diagnostics-body"><p id="support-info"></p><pre id="trace" data-i18n-aria="log"><span data-i18n="emptyLog"></span></pre><button id="clear-log" type="button" class="secondary-button"><span data-i18n="clearLog"></span></button></div></details>
      <footer class="page-footer"><span>iBasso Control</span></footer>
    </section>
  </main>`;

function element<T extends HTMLElement>(id: string): T {
    return document.getElementById(id) as T;
}
const connectButton = element<HTMLButtonElement>("connect");
const disconnectButton = element<HTMLButtonElement>("disconnect");
const refreshButton = element<HTMLButtonElement>("refresh");
const notice = element<HTMLDivElement>("notice");
const supported = window.isSecureContext && "hid" in navigator && "locks" in navigator;
let client: WebHidClient | null = null;
let device: HIDDevice | null = null;
let releaseLease: (() => void) | null = null;
let snapshot: Status | null = null;
let busy = false;
let epoch = 0;
const logs: string[] = [];
let lastNotice: { text: Key | Error; values: Record<string, unknown> } | null = null;
let syncedAt: Date | null = null;

function log(text: string): void {
    logs.push(`${new Date().toLocaleTimeString()} ${text}`);
    if (logs.length > 80) logs.shift();
    element("trace").textContent = logs.join("\n");
}
function message(text: Key | Error, error = false, values: Record<string, unknown> = {}): void {
    lastNotice = { text, values };
    notice.textContent = text instanceof Error ? text.message : t(text, values);
    notice.classList.toggle("is-error", error);
    notice.setAttribute("role", error ? "alert" : "status");
}
function renderAvailability(): void {
    const live = client !== null && snapshot !== null;
    element("hub").hidden = !live;
    document.querySelector(".workspace")?.classList.toggle("has-device", live);
    disconnectButton.hidden = !live;
    connectButton.hidden = live;
    connectButton.disabled = !supported || busy || client !== null;
    disconnectButton.disabled = client === null || busy;
    refreshButton.disabled = !live || busy;
    for (const fields of document.querySelectorAll("fieldset")) fields.disabled = !live || busy;
    element("connection-badge").textContent = t(busy ? "busy" : live ? "connected" : "disconnected");
    element("connection-badge").classList.toggle("is-live", live);
    document.querySelector(".device-panel")?.classList.toggle("is-connected", live);
    app.setAttribute("aria-busy", String(busy));
}
function clearReadings(): void {
    element("firmware").textContent = "—";
    syncedAt = null;
    element("updated").textContent = t("waiting");
    for (const name of ["filter", "gain", "output", "eco"])
        element(`${name}-current`).textContent = t("current", { value: "—" });
    element("volume-current").textContent = t("volumeEmpty");
}
function named(raw: number, table: Record<string, number>, names: Record<string, string>): string {
    const key = Object.keys(table).find((key) => table[key] === raw);
    return key ? names[key] : t("unknown", { value: hex(raw) });
}
function getNames(): Record<string, string> {
    return Object.fromEntries(
        ["d1", "d2", "d3", "d4", "d5", "low", "medium", "high", "normal", "turbo", "on", "off"].map((key) => [
            key,
            key.startsWith("d") ? key.toUpperCase() + " · " + t(key as Key) : t(key as Key),
        ]),
    );
}
function showPair(kind: string, left: number, right: number, table: Record<string, number>, seedDraft = true): void {
    const names = getNames();
    element(`${kind}-current`).textContent =
        left === right
            ? t("current", { value: named(left, table, names) })
            : t("pair", { left: named(left, table, names), right: named(right, table, names) });
    if (seedDraft)
        element<HTMLSelectElement>(kind).value =
            left === right ? (Object.keys(table).find((key) => table[key] === left) ?? "") : "";
}
function updateDraft(): void {
    const input = element<HTMLInputElement>("volume-level");
    const valid = input.value !== "" && input.validity.valid;
    element("volume-draft").textContent = valid ? input.value : "—";
    if (valid) element<HTMLInputElement>("volume-range").value = input.value;
    const balance = Number(element<HTMLInputElement>("balance").value);
    element("balance-draft").textContent = balance === 0 ? t("center") : `${balance > 0 ? "+" : ""}${balance}`;
}
function showStatus(status: Status, seedDraft = true): void {
    snapshot = status;
    element("firmware").textContent = `1.${status.firmware.toString(16)}`;
    const level = (raw: number) => {
        const value = volumeLevel(raw);
        return value === null ? hex(raw) : `${value}/100`;
    };
    element("volume-current").textContent = t("volumeCurrent", {
        left: level(status.volumeLeft),
        right: level(status.volumeRight),
    });
    // Only seed an unambiguous centered draft. Saturated/muted or unbalanced values
    // do not uniquely identify the App's previous volume/balance settings.
    if (seedDraft) {
        const recovered = volumeLevel(status.volumeLeft);
        element<HTMLInputElement>("volume-level").value =
            status.volumeLeft === status.volumeRight && recovered !== null ? String(recovered) : "";
        element<HTMLInputElement>("volume-range").value = element<HTMLInputElement>("volume-level").value || "0";
        element<HTMLInputElement>("balance").value = "0";
    }
    updateDraft();
    showPair("filter", status.filterLeft, status.filterRight, FILTERS, seedDraft);
    showPair("gain", status.gainLeft, status.gainRight, GAINS, seedDraft);
    showPair("output", status.outputLeft, status.outputRight, OUTPUTS, seedDraft);
    element("eco-current").textContent = t("current", { value: named(status.eco, { off: 0, on: 0x20 }, getNames()) });
    if (seedDraft) element<HTMLSelectElement>("eco").value = status.eco === 0 ? "off" : status.eco === 0x20 ? "on" : "";
    if (seedDraft) syncedAt = new Date();
    element("updated").textContent = t("synced", { time: syncedAt?.toLocaleTimeString(i18n.language) });
}
async function resetSession(): Promise<void> {
    epoch += 1;
    const old = client;
    const release = releaseLease;
    client = null;
    device = null;
    releaseLease = null;
    snapshot = null;
    clearReadings();
    renderAvailability();
    try {
        await old?.close();
    } catch (error) {
        log(t("closeError", { detail: asError(error).message }));
    } finally {
        release?.();
        busy = false;
        renderAvailability();
    }
}
async function operation(work: () => Promise<void>): Promise<void> {
    if (busy) return;
    busy = true;
    renderAvailability();
    const token = epoch;
    try {
        await work();
    } catch (error) {
        if (token !== epoch) return;
        const text = error instanceof LocalizedError ? error : new LocalizedError("error", { detail: asError(error) });
        log(t("error", { detail: text.message }));
        await resetSession();
        message(text, true);
    } finally {
        if (token === epoch) {
            busy = false;
            renderAvailability();
        }
    }
}
connectButton.addEventListener(
    "click",
    () =>
        void operation(async () => {
            const token = epoch;
            // requestDevice runs directly within the click's user activation, before other awaits.
            const candidates = await navigator.hid.requestDevice({ filters: [DEVICE_FILTER] });
            if (candidates.length === 0) {
                message("noneSelected");
                return;
            }
            const selected = candidates.find(supportsDevice);
            if (!selected) throw new LocalizedError("wrongDevice");
            releaseLease = await acquireDeviceLease();
            device = selected;
            client = new WebHidClient(browserPort(selected), 1000, (direction, bytes) =>
                log(`${direction} ${Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join(" ")}`),
            );
            await client.open();
            if (token !== epoch) return;
            message("reading");
            const result = await client.status();
            if (token !== epoch) return;
            showStatus(result);
            message("connectedNotice");
        }),
);
disconnectButton.addEventListener("click", () => void resetSession().then(() => message("disconnectedNotice")));
refreshButton.addEventListener(
    "click",
    () =>
        void operation(async () => {
            const token = epoch;
            const result = await client!.status();
            if (token !== epoch) return;
            showStatus(result);
            message("refreshed");
        }),
);
element<HTMLInputElement>("volume-range").addEventListener("input", (event) => {
    element<HTMLInputElement>("volume-level").value = (event.target as HTMLInputElement).value;
    updateDraft();
});
element("volume-level").addEventListener("input", updateDraft);
element("balance").addEventListener("input", updateDraft);
function bindForm(id: string, setting: () => Setting): void {
    element<HTMLFormElement>(`${id}-form`).addEventListener("submit", (event) => {
        event.preventDefault();
        if (!client || !snapshot || busy) return;
        const form = event.currentTarget as HTMLFormElement;
        if (!form.reportValidity()) return;
        // Capture the draft before disabling its fieldset for the pending operation.
        const selected = setting();
        void operation(async () => {
            const token = epoch;
            const current = client!;
            message("applying");
            await current.apply(selected);
            const result = await current.status();
            if (token !== epoch) return;
            showStatus(result);
            message("applied");
        });
    });
}
bindForm("volume", () => ({
    kind: "volume",
    level: Number(element<HTMLInputElement>("volume-level").value),
    balance: Number(element<HTMLInputElement>("balance").value),
}));
bindForm("filter", () => ({
    kind: "filter",
    value: element<HTMLSelectElement>("filter").value as keyof typeof FILTERS,
}));
bindForm("gain", () => ({ kind: "gain", value: element<HTMLSelectElement>("gain").value as keyof typeof GAINS }));
bindForm("output", () => ({
    kind: "output",
    value: element<HTMLSelectElement>("output").value as keyof typeof OUTPUTS,
}));
bindForm("eco", () => ({ kind: "eco", value: element<HTMLSelectElement>("eco").value === "on" }));
element("clear-log").addEventListener("click", () => {
    logs.length = 0;
    element("trace").textContent = t("emptyLog");
});
if ("hid" in navigator)
    navigator.hid.addEventListener("disconnect", (event) => {
        if (event.device !== device) return;
        void resetSession().then(() => message("unplugged", true));
    });
window.addEventListener("pagehide", () => {
    void resetSession();
});
const languageSelect = element<HTMLSelectElement>("language");
for (const [value, label] of Object.entries(languages)) {
    const option = document.createElement("option");
    option.value = value;
    option.textContent = label;
    languageSelect.append(option);
}
function translateUI(): void {
    document.documentElement.lang = i18n.language;
    document.title = t("title");
    languageSelect.value = i18n.language;
    for (const node of document.querySelectorAll<HTMLElement>("[data-i18n]"))
        node.textContent = t(node.dataset.i18n as Key);
    for (const node of document.querySelectorAll<HTMLElement>("[data-i18n-aria]"))
        node.setAttribute("aria-label", t(node.dataset.i18nAria as Key));
    for (const option of document.querySelectorAll<HTMLOptionElement>("[data-filter-label]")) {
        const key = option.dataset.filterLabel as Key;
        option.textContent = key.toUpperCase() + " · " + t(key);
    }
    element("support-info").textContent = t(supported ? "supported" : "unsupported");
    if (!logs.length) element("trace").textContent = t("emptyLog");
    if (snapshot) showStatus(snapshot, false);
    else clearReadings();
    updateDraft();
    if (lastNotice)
        notice.textContent =
            lastNotice.text instanceof Error ? lastNotice.text.message : t(lastNotice.text, lastNotice.values);
    renderAvailability();
}
languageSelect.addEventListener("change", () => {
    void setLanguage(languageSelect.value as Language);
});
i18n.on("languageChanged", translateUI);
if (!supported) message("browserHelp", true);
translateUI();
