import { history, historyKeymap, redo, redoDepth, undo, undoDepth } from "@codemirror/commands";
import { Compartment, EditorState, Transaction } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";
import { fitPanes, paneWidth } from "./layout.mjs";
import { dialogButtonIndex, floatingMenu } from "./menu.mjs";
import { ancestorFolders, paneDestination } from "./pane-focus.mjs";

import { displayPath, isMarkdown, pathInFolder, remapOpenDocuments, renamedPath, treeCommand, workspaceFilePath } from "./explorer.mjs";

const { invoke } = window.__TAURI__.core;
const $ = id => document.getElementById(id);
const documents = new Map();
const branches = new Map();
const expanded = new Set();
let workspace = "";
let folder = "";
let active = null;
let editor = null;
let choosing = false;
let mutating = false;
let viewEpoch = 0;
let entryAction = null;
let treeFocusPath = null;
let lastWorkspaceZone = "editor";
let lastWorkspaceControl = null;
let keyboardPaneFocus = false;
const editorLabel = new Compartment();

const status = message => { $("status").textContent = message; };
const report = error => status(String(error));
const dirty = document => document.base.text !== null && document.text !== document.base.text;
const unsaved = () => [...documents.values()].some(dirty);
const filename = path => path.split("/").at(-1);
const shownPath = path => displayPath(path, branches.get("") ?? []);

function button(label, action) {
    const element = document.createElement("button");
    element.textContent = label;
    element.onclick = () => Promise.resolve().then(action).catch(report);
    return element;
}

function renderDetails() {
    $("details").textContent = active ? active.problem ??
        `${shownPath(active.path)} · ${active.readOnly ? "Read only" : dirty(active) ? "Unsaved" : "Saved"}` :
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
        const title = button(filename(item.path), () => display(item));
        title.className = "tab-title";
        title.setAttribute("aria-label", filename(item.path) + (dirty(item) ? ", unsaved changes" : ""));
        tab.classList.toggle("active", item === active);
        tab.classList.toggle("dirty", dirty(item));
        title.title = shownPath(item.path);
        if (item === active) title.setAttribute("aria-current", "true");
        const close = button("×", () => closeTab(item));
        close.className = "tab-close";
        close.title = close.ariaLabel = `Close ${shownPath(item.path)}`;
        tab.append(title, close);
        return tab;
    }));
}

function closeTab(item) {
    if (choosing || mutating) return;
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
        ($("files").querySelector("button") ?? $("file-menu")).focus();
    }
    renderTabs();
    renderDetails();
    updateTreeSelection();
    if (active) {
        $("tabs").querySelector('[aria-current="true"]')?.focus();
    }
}

function setupPanes() {
    const layout = document.querySelector(".workspace");
    let wanted = {files: 260, details: Math.round(document.body.clientWidth * .22)};
    let sizes;
    const other = {files: "details", details: "files"};
    const update = () => {
        if (layout.hidden) return;
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
    item.editorState = null;
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

function labelFor(item) {
    return EditorView.contentAttributes.of({ "aria-label": filename(item.path), tabindex: "0" });
}

function display(item, focusEditor = true) {
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
        const cached = item.editorState;
        editor = new EditorView({
            parent: $("editor"),
            state: cached ?? EditorState.create({
                doc: text,
                extensions: [
                    EditorState.lineSeparator.of(separator), EditorState.readOnly.of(item.readOnly),
                    EditorView.editable.of(!item.readOnly), EditorView.lineWrapping,
                    editorLabel.of(labelFor(item)),
                    history(), normalizeCrlf, keymap.of([{ key: "Mod-s", run: () => { save().catch(report); return true; } }, ...historyKeymap]),
                    EditorView.updateListener.of(update => {
                        item.editorState = update.state;
                        if (!update.docChanged) return;
                        const wasDirty = dirty(item);
                        item.text = bom + update.state.sliceDoc();
                        if (wasDirty !== dirty(item)) renderTabs();
                        renderDetails();
                    }),
                ],
            }),
        });
        if (cached) editor.dispatch({effects: editorLabel.reconfigure(labelFor(item))});
        item.editorState = editor.state;
    }
    renderTabs();
    renderDetails();
    updateTreeSelection();
    if (focusEditor) editor?.focus();
}

async function open(path, kind) {
    let item = documents.get(path);
    if (!item) {
        const epoch = viewEpoch;
        const base = await invoke("read_document", {path});
        if (choosing || mutating || epoch !== viewEpoch) return;
        item = {path, kind};
        setBase(item, base);
        if (kind === "markdown") {
            try { item.recovered = await invoke("recovery_draft", {path}); }
            catch (error) { report(error); }
        }
        if (choosing || mutating || epoch !== viewEpoch) return;
        documents.set(path, item);
    }
    display(item);
}

function updateTreeSelection() {
    for (const row of $("files").querySelectorAll("[data-path]")) {
        const selected = row.dataset.path === active?.path;
        row.classList.toggle("selected", selected);
        if (selected) row.firstElementChild.setAttribute("aria-current", "true");
        else row.firstElementChild.removeAttribute("aria-current");
    }
}

const actionsForEntry = [["rename", "Rename"], ["move", "Move"], ["duplicate", "Duplicate"], ["trash", "Move to Trash"]];

function treeRows() {
    return [...$("files").querySelectorAll(".tree-row")];
}

function setTreeFocus(path, focus = false) {
    const rows = treeRows();
    const target = rows.find(row => row.dataset.path === path && !row.firstElementChild.disabled) ??
        rows.find(row => !row.firstElementChild.disabled);
    treeFocusPath = target?.dataset.path ?? null;
    for (const row of rows) {
        for (const button of row.children) button.tabIndex = row === target ? 0 : -1;
    }
    if (focus) target?.firstElementChild.focus();
    return Boolean(target);
}

$("files").addEventListener("focusin", event => {
    const row = event.target.closest(".tree-row");
    if (row && row.dataset.path !== treeFocusPath) setTreeFocus(row.dataset.path);
});

$("files").addEventListener("keydown", event => {
    if (choosing || mutating || event.altKey || event.ctrlKey || event.metaKey || event.isComposing) return;
    const row = event.target.closest(".tree-row");
    if (!row) return;
    const dom = treeRows();
    const rows = dom.map(element => ({
        path: element.dataset.path,
        disabled: element.firstElementChild.disabled,
        root: element.dataset.root === "true",
        expanded: element.firstElementChild.hasAttribute("aria-expanded") ?
            element.firstElementChild.getAttribute("aria-expanded") === "true" : undefined,
    }));
    const command = treeCommand(rows, dom.indexOf(row), event.key, event.shiftKey, event.target !== row.firstElementChild);
    if (!command) return;
    event.preventDefault();
    switch (command.kind) {
        case "focus": setTreeFocus(command.path, true); break;
        case "activate": if (!event.repeat) row.firstElementChild.click(); break;
        case "menu": if (!event.repeat) row.querySelector(".tree-more")?.click(); break;
        case "rename": case "trash": case "delete":
            if (!event.repeat) {
                const path = row.dataset.path;
                const parent = path.slice(0, Math.max(0, path.lastIndexOf("/")));
                showEntryDialog({kind: command.kind, path, parent});
            }
            break;
    }
});

function folderCreateButton(kind, parent) {
    const control = button("", () => showEntryDialog({kind, parent}));
    control.className = "icon-button tree-create";
    control.title = entryTitles[kind];
    control.setAttribute("aria-label", `${entryTitles[kind]} in ${shownPath(parent)}`);
    const icon = document.createElement("span");
    icon.className = `add-icon ${kind}`;
    icon.setAttribute("aria-hidden", "true");
    control.append(icon);
    return control;
}

function renderWorkspaceName() {
    $("folder").textContent = workspace || "No folder open";
    $("folder").title = workspace;
}

function renderTree() {
    const focused = document.activeElement?.closest("[data-path]")?.dataset.path;
    const scroll = $("files-scroll").scrollTop;
    menus.close();
    function children(path) {
        return (branches.get(path) ?? []).map(entry => {
            const child = pathInFolder(path, entry.name);
            const item = document.createElement("li");
            const row = document.createElement("div");
            row.className = "tree-row";
            row.dataset.path = child;
            const isRoot = path === "";
            row.dataset.root = String(isRoot);
            const isFolder = entry.kind === "folder" || entry.kind === "root";
            const enabled = entry.kind !== "unsupported" && entry.kind !== "unavailable";
            const openRow = button(entry.label ?? entry.name, async () => {
                if (choosing || mutating) return;
                if (!isFolder) {
                    folder = path;
                    await open(child, entry.kind);
                    return;
                }
                folder = child;
                if (expanded.delete(child)) return renderTree();
                expanded.add(child);
                const epoch = viewEpoch;
                try {
                    const entries = await invoke("list_directory", {path: child});
                    if (epoch !== viewEpoch || !expanded.has(child)) return;
                    branches.set(child, entries);
                    renderTree();
                } catch (error) {
                    if (epoch === viewEpoch) expanded.delete(child);
                    throw error;
                }
            });
            openRow.title = entry.kind === "unavailable" ? `Unavailable: ${entry.hint}` : entry.hint ?? shownPath(child);
            openRow.className = `tree-name ${isFolder ? "folder" : entry.kind}`;
            if (isFolder) openRow.setAttribute("aria-expanded", String(expanded.has(child)));
            openRow.disabled = !enabled;
            row.append(openRow);
            if (isFolder) row.append(folderCreateButton("file", child), folderCreateButton("folder", child));
            if (enabled && !isRoot) {
                const more = button("⋯", () => menus.open(more, actionsForEntry.map(([kind, label]) => ({
                    label, run: () => showEntryDialog({kind, path: child, parent: path}),
                }))));
                more.className = "tree-more";
                more.setAttribute("aria-label", `Actions for ${shownPath(child)}`);
                more.setAttribute("aria-haspopup", "menu");
                more.setAttribute("aria-expanded", "false");
                more.setAttribute("aria-controls", "floating-menu");
                row.append(more);
            }
            item.append(row);
            if (isFolder && expanded.has(child)) {
                const nested = document.createElement("ul");
                nested.append(...children(child));
                item.append(nested);
            }
            return item;
        });
    }
    renderWorkspaceName();
    $("files").replaceChildren(...children(""));
    updateTreeSelection();
    setTreeFocus(focused ?? treeFocusPath);
    $("files-scroll").scrollTop = scroll;
    if (focused && !document.querySelector("dialog[open]")) {
        setTreeFocus(focused, true);
        $("files-scroll").scrollTop = scroll;
    }
}

async function reloadTree() {
    const epoch = viewEpoch;
    const root = await invoke("list_directory", {path: ""});
    if (epoch !== viewEpoch) return;
    const available = root.filter(entry => entry.kind === "root");
    const previouslyAvailable = new Set((branches.get("") ?? [])
        .filter(entry => entry.kind === "root").map(entry => entry.name));
    for (const entry of available) {
        if (!previouslyAvailable.has(entry.name)) expanded.add(entry.name);
    }
    const descendants = await Promise.all([...expanded].map(async path => {
        try { return [path, await invoke("list_directory", {path})]; }
        catch { return [path, null]; }
    }));
    if (epoch !== viewEpoch) return;
    branches.clear();
    branches.set("", root);
    if (!available.some(entry => entry.name === folder.split("/")[0])) {
        folder = available[0]?.name ?? "";
    }
    for (const [path, entries] of descendants) {
        if (entries && expanded.has(path)) branches.set(path, entries);
        if (!entries) expanded.delete(path);
    }
    renderTree();
}

async function save(base = active?.base) {
    const item = active;
    if (choosing || mutating || !item || item.readOnly || item.saving || !dirty(item)) return;
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
    if (choosing || mutating) return;
    const epoch = viewEpoch;
    for (const item of documents.values()) {
        if (item.saving) continue;
        const base = item.base;
        const path = item.path;
        try {
            const disk = await invoke("read_document", {path});
            if (epoch !== viewEpoch || mutating) return;
            if (item.saving || item.base !== base) continue;
            if (disk.revision === base.revision) {
                item.conflict = null;
                continue;
            }
            if (dirty(item)) item.conflict = disk;
            else {
                setBase(item, disk);
                item.conflict = null;
                if (item === active) display(item, editor?.hasFocus ?? false);
            }
        } catch (error) {
            if (epoch !== viewEpoch || mutating) return;
            item.problem = String(error);
        }
    }
    renderDetails();
}

const entryTitles = {
    rename: "Rename", move: "Move", duplicate: "Duplicate", trash: "Move to Trash", delete: "Delete permanently",
    folder: "New folder", file: "New file", workspace: "Rename workspace",
};

function showEntryDialog(action) {
    if (choosing || mutating || !workspace) return;
    entryAction = {parent: folder, ...action};
    const {kind, path} = action;
    const creating = kind === "folder" || kind === "file";
    if (creating && !entryAction.parent) return;
    const move = kind === "move";
    const trash = kind === "trash";
    const deleting = trash || kind === "delete";
    $("entry-heading").textContent = entryTitles[kind];
    $("entry-submit").textContent = creating ? "Create" : kind === "workspace" ? "Rename" : entryTitles[kind];
    $("entry-description").hidden = kind === "rename" || kind === "workspace";
    $("entry-description").textContent = move ? "Enter a folder path relative to this root folder. Leave blank to move to the root." :
        trash ? `Move ${shownPath(path)} to this workspace's Trash?` :
        kind === "delete" ? `Permanently delete ${shownPath(path)} and any contents? This cannot be undone.` :
        kind === "duplicate" ? "Duplicate the saved version in this folder." :
        creating ? `Create in ${shownPath(entryAction.parent)}` : "";
    $("entry-field").hidden = deleting;
    $("entry-label").textContent = move ? "Destination folder" : "Name";
    $("entry-name").required = !(move || deleting);
    $("entry-name").value = kind === "workspace" ? workspace :
        kind === "rename" ? filename(path) : kind === "file" ? "untitled.md" : "";
    $("entry-error").hidden = true;
    $("entry-dialog").showModal();
    if (deleting) $("entry-cancel").focus();
    else $("entry-name").select();
}

async function changeEntry(action, name) {
    const {kind, path, parent} = action;
    if (kind === "folder" || kind === "file") {
        const warning = await invoke("create_entry", {parent, name, folder: kind === "folder"});
        folder = parent;
        expanded.add(parent);
        return warning;
    }
    if (kind === "trash" || kind === "delete") {
        if (affectedTabs(path).length) {
            throw new Error("Close the affected tabs before deleting this entry.");
        }
        const rows = treeRows();
        const index = rows.findIndex(row => row.dataset.path === path);
        const survives = row => !row.firstElementChild.disabled &&
            renamedPath(row.dataset.path, path, "") === row.dataset.path;
        const neighbor = rows.slice(index + 1).find(survives) ?? rows.slice(0, index).reverse().find(survives);
        const warning = await invoke(kind === "trash" ? "trash_entry" : "delete_entry", {path});
        treeFocusPath = neighbor?.dataset.path ?? null;
        for (const open of expanded) {
            if (renamedPath(open, path, "") !== open) expanded.delete(open);
        }
        if (renamedPath(folder, path, "") !== folder) folder = parent;
        return warning;
    }
    if (kind === "duplicate") return invoke("duplicate_entry", {path, name});

    const segments = name.split("/");
    if (kind === "rename" && (!name || segments.length !== 1) ||
        name && segments.some(part => !part || part === "." || part === "..")) {
        throw new Error(kind === "rename" ? "Enter a single file or folder name." :
            "Enter a root-relative folder path without a leading or trailing slash.");
    }
    const root = path.split("/")[0];
    const destinationFolder = name ? pathInFolder(root, name) : root;
    const destination = kind === "move" ? pathInFolder(destinationFolder, filename(path)) : pathInFolder(parent, name);
    if (affectedTabs(path).some(open => open.split("/")[0] !== root)) {
        throw new Error("Close affected tabs opened through another root before moving or renaming this entry.");
    }
    const entries = remapOpenDocuments(documents, path, destination);
    if (kind === "rename" && documents.has(path) && isMarkdown(path) !== isMarkdown(destination)) {
        throw new Error("Close the file before changing between Markdown and a reference file.");
    }
    const warning = kind === "move" ? await invoke("move_entry", {path, folder: destinationFolder}) :
        await invoke("rename_entry", {path, name});
    documents.clear();
    for (const [newPath, item] of entries) {
        item.path = newPath;
        documents.set(newPath, item);
    }
    if (editor) editor.dispatch({effects: editorLabel.reconfigure(labelFor(active))});
    const branchesAfterMove = [...expanded].map(open => renamedPath(open, path, destination));
    expanded.clear();
    for (const open of branchesAfterMove) expanded.add(open);
    folder = renamedPath(folder, path, destination);
    if (treeFocusPath) treeFocusPath = renamedPath(treeFocusPath, path, destination);
    renderTabs();
    renderDetails();
    return warning;
}

function affectedTabs(path) {
    const roots = branches.get("") ?? [];
    const target = workspaceFilePath(path, roots);
    return [...documents.keys()].filter(open => {
        const file = workspaceFilePath(open, roots);
        return target && file ? file === target || file.startsWith(target + "/") :
            open === path || open.startsWith(path + "/");
    });
}

async function renderTrash() {
    const entries = await invoke("list_trash");
    $("trash-files").replaceChildren(...entries.map(entry => {
        const row = document.createElement("li");
        row.append(button(`Restore ${shownPath(entry.path)}`, () => restoreEntry(entry)));
        return row;
    }));
    if (!entries.length) $("trash-status").textContent = "Trash is empty.";
}

async function restoreEntry(entry) {
    if (mutating) return;
    mutating = true;
    viewEpoch++;
    try {
        if (affectedTabs(entry.path).length) {
            throw new Error("Close the affected tabs before restoring this entry.");
        }
        const warning = await invoke("restore_entry", {id: entry.id});
        await reloadTree();
        await renderTrash();
        $("trash-status").textContent = warning ?? `Restored ${shownPath(entry.path)}.`;
        $("trash-close").focus();
    } catch (error) {
        $("trash-status").textContent = String(error);
    } finally {
        mutating = false;
    }
}

$("trash-open").onclick = async () => {
    if (mutating || choosing) return;
    $("trash-files").replaceChildren();
    $("trash-status").textContent = "";
    $("trash-dialog").showModal();
    try { await renderTrash(); }
    catch (error) { $("trash-status").textContent = String(error); }
};
$("trash-close").onclick = () => { if (!mutating) $("trash-dialog").close(); };
$("trash-dialog").oncancel = event => { if (mutating) event.preventDefault(); };

$("workspace-rename").onclick = () => showEntryDialog({kind: "workspace"});
$("entry-cancel").onclick = () => $("entry-dialog").close();
$("entry-dialog").oncancel = event => { if (mutating) event.preventDefault(); };
$("entry-form").onsubmit = async event => {
    event.preventDefault();
    if (mutating) return;
    const error = $("entry-error");
    const {kind, parent, path} = entryAction;
    const renamingWorkspace = kind === "workspace";
    if (!renamingWorkspace && [...documents.values()].some(item => item.saving)) {
        error.textContent = "Wait for the save to finish.";
        error.hidden = false;
        return;
    }
    mutating = true;
    if (!renamingWorkspace) viewEpoch++;
    $("entry-submit").disabled = $("entry-cancel").disabled = $("entry-name").disabled = true;
    try {
        if (renamingWorkspace) {
            workspace = await invoke("rename_workspace", {name: $("entry-name").value});
            renderWorkspaceName();
            $("entry-dialog").close();
            return;
        }
        const warning = await changeEntry(entryAction, $("entry-name").value);
        $("entry-dialog").close();
        status(warning ?? "");
        await reloadTree().catch(report);
        setTreeFocus(path ? treeFocusPath ?? parent : parent, true);
    } catch (problem) {
        error.textContent = String(problem);
        error.hidden = false;
    } finally {
        mutating = false;
        $("entry-submit").disabled = $("entry-cancel").disabled = $("entry-name").disabled = false;
        if ($("entry-dialog").open) $("entry-name").focus();
    }
};

function setWorkspace(name) {
    viewEpoch++;
    workspace = name;
    $("workspace-layout").hidden = !name;
    $("workspace-home").hidden = Boolean(name);
    lastWorkspaceZone = "editor";
    lastWorkspaceControl = null;
    folder = "";
    treeFocusPath = null;
    expanded.clear();
    branches.clear();
    documents.clear();
    active = null;
    editor?.destroy();
    editor = null;
    $("editor").textContent = "Select a Markdown file.";
    renderTabs();
    renderDetails();
    renderTree();
}

function renderWorkspaceHome(saved) {
    $("saved-workspaces").replaceChildren(...saved.map(entry => {
        const tile = button("", () => selectWorkspace("open_saved_workspace", {id: entry.id}));
        const title = document.createElement("strong");
        title.textContent = entry.name;
        tile.append(title);
        for (const path of entry.roots) {
            const root = document.createElement("span");
            root.textContent = path;
            tile.append(root);
        }
        return tile;
    }));
    $("saved-workspaces").hidden = saved.length === 0;
}

async function loadWorkspaceState() {
    const {name, saved, warning} = await invoke("workspace_state");
    setWorkspace(name ?? "");
    if (name) await reloadTree();
    else renderWorkspaceHome(saved);
    if (warning) status(warning);
}

async function selectWorkspace(command, args) {
    if (choosing || mutating || [...documents.values()].some(item => item.saving)) return;
    const adding = command === "add_workspace_folders" && Boolean(workspace);
    if (!adding && unsaved() && !window.confirm("Discard your unsaved drafts and leave this workspace?")) return;
    choosing = true;
    try {
        if (command === "close_workspace") {
            await invoke(command);
            await loadWorkspaceState();
            ($("saved-workspaces").querySelector("button") ?? $("create-workspace")).focus();
            return;
        }
        const selected = await invoke(command, args);
        if (!selected) return;
        status("");
        if (adding) workspace = selected;
        else setWorkspace(selected);
        await reloadTree();
        if (!adding) setTreeFocus(treeFocusPath, true);
    } finally { choosing = false; }
}

$("create-workspace").onclick = () => selectWorkspace("create_workspace").catch(report);

async function chooseFile() {
    if (choosing || mutating || [...documents.values()].some(item => item.saving)) return;
    choosing = true;
    let selected;
    try {
        selected = await invoke("choose_file", {preserveWorkspace: unsaved()});
        if (!selected) return;
        if (selected.switched) setWorkspace(selected.workspace);
        const parents = ancestorFolders(selected.path);
        folder = parents.at(-1) ?? "";
        for (const parent of parents) expanded.add(parent);
        await reloadTree();
    } finally { choosing = false; }
    await open(selected.path, isMarkdown(selected.path) ? "markdown" : "reference");
}

async function clipboardAction(action) {
    const view = editor;
    if (!view || (action !== "copy" && active.readOnly)) return;
    const state = view.state;
    if (action === "paste") {
        const text = await invoke("clipboard_text");
        if (editor !== view || view.state !== state || !text) return;
        view.dispatch(view.state.replaceSelection(text), {userEvent: "input.paste"});
    } else {
        const {from, to} = state.selection.main;
        if (from === to) return;
        await invoke("copy_text", {text: state.sliceDoc(from, to)});
        if (action === "cut" && editor === view && view.state === state) {
            view.dispatch(view.state.replaceSelection(""), {userEvent: "delete.cut"});
        }
    }
    if (editor === view) view.focus();
}

function requestClose() {
    if (choosing || mutating || [...documents.values()].some(item => item.saving)) return status("Wait for the current operation to finish.");
    if (!unsaved() || window.confirm("Discard unsaved drafts and close Offish?")) return invoke("close_offish");
}

document.addEventListener("keydown", event => {
    if (event.altKey || event.metaKey || event.shiftKey || event.isComposing) return;
    const dialog = event.target.closest?.("dialog[open]");
    if (dialog && !event.ctrlKey && event.target.matches("button") &&
        (event.key === "ArrowLeft" || event.key === "ArrowRight")) {
        const buttons = [...dialog.querySelectorAll("button:not(:disabled)")]
            .filter(button => !button.closest("[hidden]"));
        const target = dialogButtonIndex(buttons.indexOf(event.target), buttons.length, event.key);
        if (target === null) return;
        event.preventDefault();
        buttons[target].focus();
    } else if (!dialog && event.ctrlKey && event.key.toLowerCase() === "w" &&
        active && editor?.contentDOM.contains(event.target)) {
        event.preventDefault();
        if (!event.repeat) closeTab(active);
    }
}, true);

const menus = floatingMenu($("floating-menu"), report);
function focusZone(element) {
    return element?.closest("[data-focus-zone]")?.dataset.focusZone ?? null;
}

function updatePaneHighlight() {
    const zone = keyboardPaneFocus ? focusZone(document.activeElement) : null;
    for (const element of document.querySelectorAll("[data-focus-zone]")) {
        element.classList.toggle("keyboard-pane-focus", element.dataset.focusZone === zone);
    }
}

document.addEventListener("focusin", event => {
    const zone = focusZone(event.target);
    if (zone && zone !== "menu") {
        lastWorkspaceZone = zone;
        lastWorkspaceControl = event.target;
    }
    updatePaneHighlight();
});
document.addEventListener("pointerdown", () => {
    keyboardPaneFocus = false;
    updatePaneHighlight();
}, true);

function firstDetailsButton() {
    return [...$("details-pane").querySelectorAll("button:not(:disabled)")]
        .find(button => !button.closest("[hidden]"));
}

async function focusFiles() {
    const path = active?.path;
    if (!path) {
        if (!setTreeFocus(treeFocusPath, true)) $("files-pane").focus();
        return;
    }
    const row = treeRows().find(row => row.dataset.path === path);
    if (row && !row.firstElementChild.disabled) {
        setTreeFocus(path, true);
        return;
    }
    $("files-pane").focus();
    if (!workspace) return;
    const parents = ancestorFolders(path);
    for (const parent of parents) expanded.add(parent);
    const epoch = viewEpoch;
    await reloadTree();
    // Loading a collapsed folder must not steal focus if the user moved on.
    if (document.activeElement !== $("files-pane") || epoch !== viewEpoch || active?.path !== path) return;
    setTreeFocus(path, true);
}

async function focusWorkspaceZone(zone, previous = null) {
    if (zone === "menu") return $("file-menu").focus();
    if (!workspace) return ($("saved-workspaces").querySelector("button") ?? $("create-workspace")).focus();
    if (zone === "files") {
        if (previous?.isConnected && !previous.closest("[hidden]") && !previous.disabled &&
            previous !== $("files-pane")) return previous.focus();
        return focusFiles();
    }
    if (zone === "editor") {
        if (editor) return editor.focus();
        return $("editor-pane").focus();
    }
    if (zone === "details") {
        const first = firstDetailsButton();
        if (first && previous?.isConnected && !previous.closest("[hidden]") && !previous.disabled &&
            previous.matches("button, textarea")) return previous.focus();
        return first?.focus();
    }
    $(zone)?.focus();
}

document.addEventListener("keydown", event => {
    if (!event.altKey || event.ctrlKey || event.metaKey || event.shiftKey || event.isComposing ||
        choosing || mutating || document.querySelector("dialog[open]")) return;
    const origin = event.target.closest?.("#floating-menu") ?
        document.querySelector('button[aria-controls="floating-menu"][aria-expanded="true"]') : event.target;
    const current = focusZone(origin) ?? lastWorkspaceZone;
    if (!workspace && (event.key === "ArrowLeft" || event.key === "ArrowRight")) return;
    const destination = paneDestination(current, lastWorkspaceZone, event.key);
    if (!destination) {
        if (current !== "menu" && (event.key === "ArrowLeft" || event.key === "ArrowRight")) {
            event.preventDefault();
            event.stopPropagation();
        }
        return;
    }
    event.preventDefault();
    event.stopPropagation();
    if (event.repeat || destination === "details" && !firstDetailsButton()) return;
    const previous = current === "menu" && event.key === "ArrowDown" ? lastWorkspaceControl : null;
    keyboardPaneFocus = true;
    menus.close();
    focusWorkspaceZone(destination, previous).catch(report);
    updatePaneHighlight();
}, true);

$("file-menu").onclick = () => menus.open($("file-menu"), [
    {label: "New file…", run: () => showEntryDialog({kind: "file"}), disabled: !folder},
    {label: "New folder…", run: () => showEntryDialog({kind: "folder"}), disabled: !folder},
    null,
    {label: "Open file…", run: chooseFile},
    {label: "Open folder…", run: () => selectWorkspace("choose_folder")},
    {label: "Add Folder to Workspace…", run: () => selectWorkspace("add_workspace_folders")},
    {label: "Create Workspace…", run: () => selectWorkspace("create_workspace")},
    {label: "Close Workspace", run: () => selectWorkspace("close_workspace"), disabled: !workspace},
    null,
    {label: "Save", run: () => save(), disabled: !active || active.readOnly || active.saving || !dirty(active)},
    {label: "Close tab", run: () => closeTab(active), disabled: !active},
    {label: "Exit", run: requestClose},
]);
$("edit-menu").onclick = () => {
    const editable = Boolean(editor && active && !active.readOnly);
    const selection = Boolean(editor && !editor.state.selection.main.empty);
    menus.open($("edit-menu"), [
        {label: "Undo", run: () => { undo(editor); editor.focus(); }, disabled: !editable || !undoDepth(editor.state)},
        {label: "Redo", run: () => { redo(editor); editor.focus(); }, disabled: !editable || !redoDepth(editor.state)},
        null,
        {label: "Cut", run: () => clipboardAction("cut"), disabled: !editable || !selection},
        {label: "Copy", run: () => clipboardAction("copy"), disabled: !selection},
        {label: "Paste", run: () => clipboardAction("paste"), disabled: !editable},
    ]);
};
for (const menu of [$("file-menu"), $("edit-menu")]) {
    menu.addEventListener("keydown", event => {
        if (event.key === "ArrowDown") {
            event.preventDefault();
            menu.click();
        }
    });
}
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
    active.editorState = null;
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
    Promise.resolve().then(requestClose).catch(report);
}).catch(report);
setupPanes();
await loadWorkspaceState().catch(report);
async function watch() {
    await refresh().catch(report);
    setTimeout(watch, 1500);
}
setTimeout(watch, 1500);
