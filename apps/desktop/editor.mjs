import { history, historyKeymap } from "@codemirror/commands";
import { EditorState, Transaction } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";
import { fitPanes, paneWidth } from "./layout.mjs";

const { invoke } = window.__TAURI__.core;
const $ = id => document.getElementById(id);
const documents = new Map();
let workspace = "";
let folder = "";
let active = null;
let editor = null;
let choosing = false;

const status = message => { $("status").textContent = message; };
const report = error => status(String(error));
const dirty = document => document.base.text !== null && document.text !== document.base.text;
const unsaved = () => [...documents.values()].some(dirty);
const filename = path => path.split("/").at(-1);

function button(label, action) {
    const element = document.createElement("button");
    element.textContent = label;
    element.onclick = () => Promise.resolve().then(action).catch(report);
    return element;
}

function renderDetails() {
    $("save").disabled = !active || active.readOnly || active.saving || !dirty(active);
    $("details").textContent = active ? active.problem ??
        `${active.path} · ${active.readOnly ? "Read only" : dirty(active) ? "Unsaved" : "Saved"}` :
        "Select a document.";
    $("recovery").hidden = active?.recovered == null;
    $("recover").disabled = !active || dirty(active) || active.readOnly || active.text === active.recovered;
    $("discard").disabled = Boolean(active?.saving);
    $("conflict").hidden = !active?.conflict;
    $("disk").value = active?.conflict?.text ?? "";
    $("keep").disabled = active?.conflict?.text == null || active?.saving;
    $("reload").disabled = active?.conflict?.text == null || active?.saving;
}

function renderTabs() {
    $("tabs").replaceChildren(...[...documents.values()].map(item => {
        const tab = document.createElement("div");
        tab.className = "tab";
        const title = button(filename(item.path) + (dirty(item) ? " •" : ""), () => display(item));
        title.title = item.path;
        if (item === active) title.setAttribute("aria-current", "true");
        const close = button("×", () => closeTab(item));
        close.className = "tab-close";
        close.title = close.ariaLabel = `Close ${item.path}`;
        tab.append(title, close);
        return tab;
    }));
}

function closeTab(item) {
    if (item.saving) return status("Wait for the save to finish.");
    if (dirty(item) && !window.confirm(`Discard unsaved changes to ${filename(item.path)}?`)) return;
    const tabs = [...documents.values()];
    const index = tabs.indexOf(item);
    const neighbor = tabs[index + 1] ?? tabs[index - 1];
    documents.delete(item.path);
    if (item === active && neighbor) {
        display(neighbor);
        return;
    }
    if (item === active) {
        editor?.destroy();
        editor = active = null;
        $("editor").textContent = "Select a Markdown file.";
        ($("files").querySelector("button") ?? $("choose")).focus();
    }
    renderTabs();
    renderDetails();
    if (active) {
        $("tabs").querySelector('[aria-current="true"]')?.focus();
    }
}

function setupPanes() {
    const layout = document.querySelector(".workspace");
    let wanted = {files: Math.round(layout.clientWidth * .20), details: Math.round(layout.clientWidth * .22)};
    let sizes;
    const other = {files: "details", details: "files"};
    const update = () => {
        sizes = fitPanes(layout.clientWidth, wanted.files, wanted.details);
        for (const pane of ["files", "details"]) {
            const divider = $(pane + "-divider");
            const {min, max, width} = paneWidth(layout.clientWidth, pane, sizes[other[pane]], sizes[pane]);
            layout.style.setProperty(`--${pane}-width`, `${width}px`);
            divider.setAttribute("aria-valuemin", min);
            divider.setAttribute("aria-valuemax", max);
            divider.setAttribute("aria-valuenow", width);
        }
    };
    for (const pane of ["files", "details"]) {
        const divider = $(pane + "-divider");
        const direction = pane === "files" ? 1 : -1;
        const resize = requested => {
            wanted = {...sizes, [pane]: paneWidth(layout.clientWidth, pane, sizes[other[pane]], requested).width};
            update();
        };
        let drag;
        divider.addEventListener("pointerdown", event => {
            if (event.button !== 0 || drag) return;
            divider.setPointerCapture(event.pointerId);
            drag = {id: event.pointerId, x: event.clientX, width: sizes[pane]};
        });
        divider.addEventListener("pointermove", event => {
            if (drag?.id === event.pointerId) resize(drag.width + direction * (event.clientX - drag.x));
        });
        divider.addEventListener("lostpointercapture", () => { drag = null; });
        divider.addEventListener("keydown", event => {
            const adjustment = {ArrowLeft: -16 * direction, ArrowRight: 16 * direction}[event.key];
            if (adjustment === undefined) return;
            event.preventDefault();
            resize(sizes[pane] + adjustment);
        });
    }
    update();
    new ResizeObserver(update).observe(layout);
}

function setBase(item, base) {
    item.base = base;
    item.text = base.text;
    const text = base.text ?? "";
    const unsupported = /\r(?!\n)/.test(text) ||
        (/\r\n/.test(text) && /(?<!\r)\n/.test(text));
    item.readOnly = item.kind !== "markdown" || base.text === null || unsupported;
    item.problem = item.kind === "markdown" && unsupported ?
        "Mixed or standalone CR line endings. This file is read only." : base.reason;
}

const normalizeCrlf = EditorState.transactionFilter.of(transaction => {
    const { lineBreak } = transaction.startState;
    if (!transaction.docChanged || lineBreak === "\n" ||
        !(transaction.isUserEvent("input") || transaction.isUserEvent("paste"))) return transaction;
    let changed = false;
    const changes = [];
    transaction.changes.iterChanges((from, to, _fromB, _toB, inserted) => {
        const text = inserted.sliceString(0, inserted.length, lineBreak);
        const insert = text.replace(/\r\n|\r|\n/g, lineBreak);
        changed ||= insert !== text;
        changes.push({ from, to, insert });
    });
    if (!changed) return transaction;
    // Each bare break becomes one separator, so positions and the selection are unchanged.
    return {
        changes, selection: transaction.selection, scrollIntoView: transaction.scrollIntoView,
        userEvent: transaction.annotation(Transaction.userEvent),
    };
});

function display(item) {
    editor?.destroy();
    $("editor").replaceChildren();
    active = item;
    if (item.text === null) {
        const message = document.createElement("p");
        message.textContent = item.base.reason;
        $("editor").append(message);
        editor = null;
    } else {
        const bom = item.text.startsWith("\uFEFF") ? "\uFEFF" : "";
        const text = item.text.slice(bom.length);
        const separator = /\r\n/.test(text) && !/\r(?!\n)|(?<!\r)\n/.test(text) ? "\r\n" : "\n";
        editor = new EditorView({
            parent: $("editor"),
            state: EditorState.create({
                doc: text,
                extensions: [
                    EditorState.lineSeparator.of(separator), EditorState.readOnly.of(item.readOnly),
                    EditorView.editable.of(!item.readOnly), EditorView.lineWrapping,
                    EditorView.contentAttributes.of({ "aria-label": filename(item.path), tabindex: "0" }),
                    history(), normalizeCrlf, keymap.of([{ key: "Mod-s", run: () => { save().catch(report); return true; } }, ...historyKeymap]),
                    EditorView.updateListener.of(update => {
                        if (!update.docChanged) return;
                        const wasDirty = dirty(item);
                        item.text = bom + update.state.sliceDoc();
                        if (wasDirty !== dirty(item)) renderTabs();
                        renderDetails();
                    }),
                ],
            }),
        });
    }
    renderTabs();
    renderDetails();
    editor?.focus();
}

async function open(path, kind) {
    let item = documents.get(path);
    if (!item) {
        const owner = workspace;
        const base = await invoke("read_document", {path});
        if (choosing || owner !== workspace) return;
        item = {path, kind};
        setBase(item, base);
        if (kind === "markdown") {
            try { item.recovered = await invoke("recovery_draft", {path}); }
            catch (error) { report(error); }
        }
        if (choosing || owner !== workspace) return;
        documents.set(path, item);
    }
    display(item);
}

async function list(path) {
    const owner = workspace;
    const entries = await invoke("list_directory", {path});
    if (owner !== workspace) return;
    folder = path;
    $("folder").textContent = path || workspace;
    $("up").hidden = !path;
    $("files").replaceChildren(...entries.map(entry => {
        const child = path ? path + "/" + entry.name : entry.name;
        const row = document.createElement("li");
        row.append(button(entry.name + (entry.kind === "folder" ? "/" : ""),
            () => entry.kind === "folder" ? list(child) : open(child, entry.kind)));
        return row;
    }));
}

async function save(base = active?.base) {
    const item = active;
    if (choosing || !item || item.readOnly || item.saving || !dirty(item)) return;
    const text = item.text;
    item.saving = true;
    renderDetails();
    try {
        const result = await invoke("save_document", {path: item.path, base: base.revision, text});
        if (result.status === "conflict") {
            item.conflict = result.disk;
            status("Save stopped because the disk version changed.");
        } else {
            item.base = {text, revision: result.revision};
            item.conflict = null;
            item.problem = null;
            item.recovered = null;
            status("Saved");
        }
    } catch (error) {
        try { item.recovered = await invoke("recovery_draft", {path: item.path}); } catch { /* Keep the save error. */ }
        throw error;
    } finally {
        item.saving = false;
        renderTabs();
        renderDetails();
    }
}

async function refresh() {
    if (choosing) return;
    const owner = workspace;
    for (const item of documents.values()) {
        if (item.saving) continue;
        const base = item.base;
        try {
            const disk = await invoke("read_document", {path: item.path});
            if (owner !== workspace || item.saving || item.base !== base) continue;
            if (disk.revision === base.revision) {
                item.conflict = null;
                continue;
            }
            if (dirty(item)) item.conflict = disk;
            else {
                setBase(item, disk);
                item.conflict = null;
                if (item === active) display(item);
            }
        } catch (error) {
            item.problem = String(error);
        }
    }
    renderDetails();
}

$("choose").onclick = async () => {
    if (choosing || [...documents.values()].some(item => item.saving)) return;
    if (unsaved() && !window.confirm("Discard your unsaved drafts and open another folder?")) return;
    choosing = true;
    try {
        const selected = await invoke("choose_folder");
        if (!selected) return;
        workspace = selected;
        documents.clear();
        active = null;
        editor?.destroy();
        editor = null;
        $("editor").textContent = "Select a Markdown file.";
        renderTabs();
        renderDetails();
        await list("");
    } catch (error) { report(error); }
    finally { choosing = false; }
};
$("up").onclick = () => list(folder.split("/").slice(0, -1).join("/")).catch(report);
$("save").onclick = () => save().catch(report);
$("keep").onclick = () => {
    if (active?.conflict && window.confirm("Replace the disk version with your draft?")) save(active.conflict).catch(report);
};
$("reload").onclick = () => {
    if (!active?.conflict || !window.confirm("Discard your draft and load the disk version?")) return;
    setBase(active, active.conflict);
    active.conflict = null;
    display(active);
};
$("recover").onclick = () => {
    if (!active?.recovered || dirty(active)) return;
    active.text = active.recovered;
    display(active);
    status("Recovered draft loaded. Review it before saving.");
};
$("discard").onclick = async () => {
    const item = active;
    if (item?.recovered == null || !window.confirm("Discard the saved recovery copy?")) return;
    try {
        await invoke("discard_recovery", {path: item.path, draft: item.recovered});
        item.recovered = null;
        renderDetails();
    } catch (error) { report(error); }
};
window.__TAURI__.event.listen("offish-close-request", () => {
    if ([...documents.values()].some(item => item.saving)) return status("Wait for the save to finish.");
    if (!unsaved() || window.confirm("Discard unsaved drafts and close Offish?")) invoke("close_offish").catch(report);
}).catch(report);
setupPanes();
workspace = await invoke("workspace_name").catch(report) || "";
if (workspace) await list("").catch(report);
async function watch() {
    await refresh().catch(report);
    setTimeout(watch, 1500);
}
setTimeout(watch, 1500);
