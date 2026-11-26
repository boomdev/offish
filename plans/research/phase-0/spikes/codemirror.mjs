import { history, historyKeymap, insertNewline } from "@codemirror/commands";
import { markdown, commonmarkLanguage } from "@codemirror/lang-markdown";
import { yamlFrontmatter } from "@codemirror/lang-yaml";
import { syntaxTree, ensureSyntaxTree } from "@codemirror/language";
import { Compartment, EditorState, StateEffect, StateField, Transaction } from "@codemirror/state";
import { Decoration, EditorView, ViewPlugin, WidgetType, keymap } from "@codemirror/view";
import { Table } from "@lezer/markdown";

// Offish Markdown profile candidate: CommonMark, pipe tables and YAML front matter.
const language = yamlFrontmatter({ content: markdown({ base: commonmarkLanguage, extensions: [Table] }) });
const encoder = new TextEncoder();
// Browser checks use the offline fixture host. The Tauri host sets window.offishAssetBase
// to a workspace-scoped custom protocol before mounting. Resolve at image-create time so
// the host can set the base after this module loads.
const assetBase = () => window.offishAssetBase || "https://offish.invalid/";

// Track per-line UTF-8 byte lengths incrementally so typing in large documents stays O(1)
// in document text instead of re-encoding the entire prefix on every keystroke.
const lineByteLength = (doc, lineNumber, lineBreakBytes) =>
    encoder.encode(doc.line(lineNumber).text).length + (lineNumber < doc.lines ? lineBreakBytes : 0);

function buildByteIndex(doc, lineBreakBytes) {
    const lengths = new Uint32Array(doc.lines);
    const prefix = new Uint32Array(doc.lines + 1);
    for (let i = 1; i <= doc.lines; i++) {
        const length = lineByteLength(doc, i, lineBreakBytes);
        lengths[i - 1] = length;
        prefix[i] = prefix[i - 1] + length;
    }
    return { lineBreakBytes, lengths, prefix };
}

const byteIndex = StateField.define({
    create: state => buildByteIndex(state.doc, encoder.encode(state.lineBreak).length),
    update(value, transaction) {
        if (!transaction.docChanged) return value;
        const oldDoc = transaction.startState.doc;
        const newDoc = transaction.state.doc;
        const { lineBreakBytes } = value;
        let firstFromA = Infinity, lastToA = -1, firstFromB = Infinity, lastToB = -1;
        transaction.changes.iterChangedRanges((fromA, toA, fromB, toB) => {
            if (fromA < firstFromA) firstFromA = fromA;
            if (toA > lastToA) lastToA = toA;
            if (fromB < firstFromB) firstFromB = fromB;
            if (toB > lastToB) lastToB = toB;
        });
        if (firstFromA === Infinity) return value;
        const startLine = oldDoc.lineAt(firstFromA).number;
        const endLineA = oldDoc.lineAt(lastToA).number;
        const endLineB = newDoc.lineAt(lastToB).number;
        const lengths = new Uint32Array(newDoc.lines);
        lengths.set(value.lengths.subarray(0, startLine - 1), 0);
        for (let i = startLine; i <= endLineB; i++) {
            lengths[i - 1] = lineByteLength(newDoc, i, lineBreakBytes);
        }
        if (endLineA < oldDoc.lines) {
            lengths.set(value.lengths.subarray(endLineA), endLineB);
        }
        const prefix = new Uint32Array(newDoc.lines + 1);
        prefix.set(value.prefix.subarray(0, startLine), 0);
        for (let i = startLine - 1; i < newDoc.lines; i++) {
            prefix[i + 1] = prefix[i] + lengths[i];
        }
        return { lineBreakBytes, lengths, prefix };
    },
});

function utf8Offset(state, offset) {
    const index = state.field(byteIndex);
    const line = state.doc.lineAt(offset);
    const column = offset - line.from;
    return index.prefix[line.number - 1] + (column ? encoder.encode(line.text.slice(0, column)).length : 0);
}

function utf16Offset(state, bytes) {
    const index = state.field(byteIndex);
    const total = index.prefix[state.doc.lines];
    if (bytes < 0 || bytes > total) throw new Error("Byte offset is out of bounds");
    if (bytes === total) return state.doc.length;
    let low = 1, high = state.doc.lines;
    while (low < high) {
        const middle = (low + high + 1) >> 1;
        if (index.prefix[middle - 1] <= bytes) low = middle; else high = middle - 1;
    }
    const line = state.doc.line(low);
    const target = bytes - index.prefix[low - 1];
    let columnLow = 0, columnHigh = line.length;
    while (columnLow < columnHigh) {
        const middle = (columnLow + columnHigh) >> 1;
        if (encoder.encode(line.text.slice(0, middle)).length < target) columnLow = middle + 1;
        else columnHigh = middle;
    }
    if (encoder.encode(line.text.slice(0, columnLow)).length !== target) {
        throw new Error("Byte offset is not on a character boundary");
    }
    return line.from + columnLow;
}

function lineEnding(source) {
    const crlf = (source.match(/\r\n/g) || []).length;
    const lf = (source.match(/\n/g) || []).length - crlf;
    const cr = (source.match(/\r(?!\n)/g) || []).length;
    // Mixed endings keep any CR as text so the bytes survive unchanged.
    if (crlf && !lf && !cr) return "\r\n";
    return "\n";
}

// Typed and pasted line breaks follow the file's convention. A CRLF document otherwise
// gains bare LF characters, because only the configured separator splits lines.
const normaliseInput = EditorState.transactionFilter.of(transaction => {
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

const touchesSelection = (state, from, to) =>
    state.selection.ranges.some(range => range.from <= to && range.to >= from);

class ImageWidget extends WidgetType {
    constructor(alt, src) { super(); this.alt = alt; this.src = src; }
    eq(other) { return other.alt === this.alt && other.src === this.src; }
    toDOM() {
        const image = document.createElement("img");
        image.alt = this.alt;
        image.src = new URL(this.src, assetBase()).href;
        return image;
    }
}

class BulletWidget extends WidgetType {
    eq() { return true; }
    toDOM() {
        const bullet = document.createElement("span");
        bullet.className = "cm-bullet";
        bullet.setAttribute("aria-hidden", "true");
        bullet.textContent = "• ";
        return bullet;
    }
}

class TableWidget extends WidgetType {
    constructor(rows, from) { super(); this.rows = rows; this.from = from; }
    eq(other) { return JSON.stringify(other.rows) === JSON.stringify(this.rows); }
    toDOM(view) {
        const table = document.createElement("table");
        table.className = "cm-table-preview";
        this.rows.forEach((cells, index) => {
            const row = table.insertRow();
            for (const text of cells) {
                const cell = document.createElement(index === 0 ? "th" : "td");
                cell.textContent = text;
                row.append(cell);
            }
        });
        // Entering the table reveals its source for editing.
        table.addEventListener("mousedown", event => {
            event.preventDefault();
            view.dispatch({ selection: { anchor: this.from } });
            view.focus();
        });
        return table;
    }
    ignoreEvent() { return false; }
}

class InsertionWidget extends WidgetType {
    constructor(text) { super(); this.text = text; }
    eq(other) { return other.text === this.text; }
    toDOM() {
        const insertion = document.createElement("ins");
        const label = document.createElement("span");
        label.className = "cm-sr-only";
        label.textContent = "Added: ";
        insertion.append(label, this.text);
        return insertion;
    }
}

function tableRows(state, node) {
    const rows = [];
    for (let child = node.firstChild; child; child = child.nextSibling) {
        if (child.name !== "TableHeader" && child.name !== "TableRow") continue;
        const cells = [];
        for (let cell = child.firstChild; cell; cell = cell.nextSibling) {
            if (cell.name === "TableCell") cells.push(state.sliceDoc(cell.from, cell.to).trim());
        }
        rows.push(cells);
    }
    return rows;
}

// Inline syntax is hidden outside the lines holding the selection.
const inlinePreview = ViewPlugin.fromClass(class {
    constructor(view) { this.decorations = this.build(view); }
    update(update) {
        if (update.docChanged || update.selectionSet || update.viewportChanged ||
            syntaxTree(update.startState) !== syntaxTree(update.state)) {
            this.decorations = this.build(update.view);
        }
    }
    build(view) {
        const { state } = view;
        const ranges = [];
        for (const { from, to } of view.visibleRanges) {
            syntaxTree(state).iterate({ from, to, enter: node => {
                const line = state.doc.lineAt(node.from);
                const endLine = state.doc.lineAt(node.to);
                const active = touchesSelection(state, line.from, endLine.to);
                switch (node.name) {
                case "Frontmatter":
                    for (let n = line.number; n <= endLine.number; n++) {
                        ranges.push(Decoration.line({ class: "cm-frontmatter" }).range(state.doc.line(n).from));
                    }
                    return false;
                case "CommentBlock":
                    for (let n = line.number; n <= endLine.number; n++) {
                        ranges.push(Decoration.line({ class: "cm-comment" }).range(state.doc.line(n).from));
                    }
                    return false;
                case "Table":
                    return false;
                case "ATXHeading1": case "ATXHeading2": case "ATXHeading3":
                    ranges.push(Decoration.line({ class: "cm-heading " + node.name }).range(line.from));
                    return true;
                case "HeaderMark":
                    if (!active) {
                        const after = state.sliceDoc(node.to, node.to + 1) === " " ? node.to + 1 : node.to;
                        ranges.push(Decoration.replace({}).range(node.from, after));
                    }
                    return false;
                case "StrongEmphasis":
                    ranges.push(Decoration.mark({ class: "cm-strong" }).range(node.from, node.to));
                    return true;
                case "Emphasis":
                    ranges.push(Decoration.mark({ class: "cm-emphasis" }).range(node.from, node.to));
                    return true;
                case "EmphasisMark": case "LinkMark":
                    if (!active) ranges.push(Decoration.replace({}).range(node.from, node.to));
                    return false;
                case "URL":
                    if (!active && node.node.parent?.name === "Link") {
                        ranges.push(Decoration.replace({}).range(node.from, node.to));
                    }
                    return false;
                case "Link":
                    ranges.push(Decoration.mark({ class: "cm-link" }).range(node.from, node.to));
                    return true;
                case "ListMark":
                    if (!active) {
                        const after = state.sliceDoc(node.to, node.to + 1) === " " ? node.to + 1 : node.to;
                        ranges.push(Decoration.replace({ widget: new BulletWidget() }).range(node.from, after));
                    }
                    return false;
                case "QuoteMark":
                    ranges.push(Decoration.line({ class: "cm-blockquote" }).range(line.from));
                    if (!active) {
                        const after = state.sliceDoc(node.to, node.to + 1) === " " ? node.to + 1 : node.to;
                        ranges.push(Decoration.replace({}).range(node.from, after));
                    }
                    return false;
                case "Image":
                    if (!active) {
                        const source = state.sliceDoc(node.from, node.to);
                        const match = /^!\[([^\]]*)\]\(([^)\s]+)\)$/.exec(source);
                        if (match) {
                            ranges.push(Decoration.replace({ widget: new ImageWidget(match[1], match[2]) })
                                .range(node.from, node.to));
                        }
                    }
                    return false;
                }
                return true;
            } });
        }
        return Decoration.set(ranges, true);
    }
}, { decorations: plugin => plugin.decorations });

function enclosingTable(tree, pos) {
    for (const side of [0, -1]) {
        for (let node = tree.resolveInner(pos, side); node; node = node.parent) {
            if (node.name === "Table") return node;
        }
    }
    return null;
}

function tableDecorationsIn(state, from, to) {
    const ranges = [];
    syntaxTree(state).iterate({ from, to, enter: node => {
        if (node.name !== "Table") return !["Paragraph", "Frontmatter", "CommentBlock"].includes(node.name);
        if (!touchesSelection(state, node.from, node.to)) {
            ranges.push(Decoration.replace({
                widget: new TableWidget(tableRows(state, node.node), node.from), block: true,
            }).range(node.from, node.to));
        }
        return false;
    } });
    return ranges;
}

// Block widgets that replace line breaks must come from state. Map existing decorations
// across edits and inspect only changed, selected, and newly parsed ranges.
const tablePreview = StateField.define({
    create(state) {
        const tree = syntaxTree(state);
        return {
            decorations: Decoration.set(tableDecorationsIn(state, 0, tree.length), true),
            parsedUpTo: tree.length,
        };
    },
    update(value, transaction) {
        const oldTree = syntaxTree(transaction.startState);
        const newTree = syntaxTree(transaction.state);
        if (!transaction.docChanged && !transaction.selection && oldTree === newTree) return value;

        let decorations = transaction.docChanged ? value.decorations.map(transaction.changes) : value.decorations;
        const { state } = transaction;
        const dirty = [];
        const addDirty = (from, to) => {
            const start = Math.max(0, from);
            const end = Math.min(state.doc.length, to);
            const startTable = enclosingTable(newTree, start);
            const endTable = enclosingTable(newTree, end);
            dirty.push([
                Math.min(start, startTable ? startTable.from : start),
                Math.max(end, endTable ? endTable.to : end),
            ]);
        };

        if (transaction.docChanged) {
            transaction.changes.iterChangedRanges((_fromA, _toA, fromB, toB) => {
                const startLine = Math.max(1, state.doc.lineAt(fromB).number - 1);
                const endLine = Math.min(state.doc.lines, state.doc.lineAt(toB).number + 1);
                addDirty(state.doc.line(startLine).from, state.doc.line(endLine).to);
            });
        }
        if (transaction.selection || transaction.docChanged) {
            for (const range of transaction.startState.selection.ranges) {
                addDirty(transaction.changes.mapPos(range.from, -1), transaction.changes.mapPos(range.to, 1));
            }
            for (const range of state.selection.ranges) {
                addDirty(range.from, range.to);
            }
        }
        if (newTree.length > value.parsedUpTo) {
            addDirty(value.parsedUpTo, newTree.length);
        }
        if (!dirty.length) return { decorations, parsedUpTo: newTree.length };

        dirty.sort((a, b) => a[0] - b[0]);
        const merged = [];
        for (const span of dirty) {
            if (!merged.length || span[0] > merged[merged.length - 1][1]) merged.push(span);
            else if (span[1] > merged[merged.length - 1][1]) merged[merged.length - 1][1] = span[1];
        }

        const add = [];
        let needsFilter = false;
        for (const [from, to] of merged) {
            decorations.between(from, to, () => { needsFilter = true; return false; });
            add.push(...tableDecorationsIn(state, from, to));
        }
        if (needsFilter || add.length) {
            const overlapsDirty = (from, to) => merged.some(([lo, hi]) => from <= hi && to >= lo);
            decorations = decorations.update({
                filter: (from, to) => !overlapsDirty(from, to),
                add,
                sort: true,
            });
        }
        return { decorations, parsedUpTo: newTree.length };
    },
    provide: field => EditorView.decorations.from(field, value => value.decorations),
});

const setProposal = StateEffect.define();
const proposalField = StateField.define({
    create: () => null,
    update(value, transaction) {
        for (const effect of transaction.effects) if (effect.is(setProposal)) return effect.value;
        if (value && transaction.docChanged) return { ...value, stale: true };
        return value;
    },
    provide: field => EditorView.decorations.from(field, proposal => {
        if (!proposal || proposal.stale) return Decoration.none;
        return Decoration.set([
            Decoration.mark({ tagName: "del", attributes: { "aria-label": "Removed: " + proposal.removed } })
                .range(proposal.from, proposal.to),
            Decoration.widget({ widget: new InsertionWidget(proposal.replacement), side: 1 }).range(proposal.to),
        ]);
    }),
});

const theme = EditorView.baseTheme({
    ".cm-frontmatter": { color: "#666", fontFamily: "monospace" },
    ".cm-comment": { color: "#888", fontStyle: "italic" },
    ".cm-heading": { fontWeight: "bold" },
    ".ATXHeading1": { fontSize: "1.6em" },
    ".ATXHeading2": { fontSize: "1.3em" },
    ".cm-link": { color: "#1a5fb4", textDecoration: "underline" },
    ".cm-strong": { fontWeight: "bold" },
    ".cm-emphasis": { fontStyle: "italic" },
    ".cm-blockquote": { borderLeft: "2px solid #999", paddingLeft: "8px" },
    "del": { background: "#fdd", textDecoration: "line-through" },
    "ins": { background: "#dfd", textDecoration: "underline" },
    ".cm-sr-only": { position: "absolute", width: "1px", height: "1px", overflow: "hidden", clip: "rect(0 0 0 0)" },
    ".cm-table-preview td, .cm-table-preview th": { border: "1px solid #999", padding: "2px 8px" },
});

function createView(parent, source, { label, separator, preview = true }) {
    const operations = [];
    const historySlot = new Compartment();
    const extensions = [
        language, theme, byteIndex, proposalField, normaliseInput, historySlot.of(history()),
        keymap.of([{ key: "Enter", run: insertNewline }, ...historyKeymap]),
        EditorView.contentAttributes.of({ "aria-label": label }),
        EditorView.updateListener.of(update => {
            if (!update.docChanged) return;
            // Typed operations against the previous revision, in UTF-8 bytes.
            update.changes.iterChanges((fromA, toA, _fromB, _toB, inserted) => {
                operations.push({
                    start: utf8Offset(update.startState, fromA),
                    end: utf8Offset(update.startState, toA),
                    replacement: inserted.sliceString(0, inserted.length, update.state.lineBreak),
                });
            });
        }),
    ];
    if (preview) extensions.push(inlinePreview, tablePreview);
    if (separator !== undefined) extensions.push(EditorState.lineSeparator.of(separator));
    const view = new EditorView({ parent, state: EditorState.create({ doc: source, extensions }) });
    return { view, operations, historySlot };
}

window.lineEndingCheck = (sources) => sources.map(source => {
    const host = document.createElement("div");
    const normalised = createView(host, source, { label: "Probe" });
    const configured = createView(host, source, { label: "Probe", separator: lineEnding(source) });
    const result = {
        defaultPreserves: normalised.view.state.sliceDoc() === source,
        configuredPreserves: configured.view.state.sliceDoc() === source,
    };
    normalised.view.destroy();
    configured.view.destroy();
    return result;
});

window.mountDocument = (source) => {
    const main = document.querySelector("main");
    const { view, operations, historySlot } = createView(main, source, {
        label: "Pilot brief", separator: lineEnding(source),
    });
    ensureSyntaxTree(view.state, view.state.doc.length, 5000);
    window.editor = view;
    const nodeNames = () => {
        const names = [];
        syntaxTree(view.state).iterate({ enter: node => { names.push(node.name); } });
        return names;
    };
    const review = document.querySelector("aside");
    return {
        source: () => view.state.sliceDoc(),
        operations: () => operations.splice(0),
        nodeNames,
        lineText(substring) {
            const line = [...view.contentDOM.querySelectorAll(".cm-line")]
                .find(element => element.textContent.includes(substring));
            return line ? line.textContent : null;
        },
        lineClass(substring) {
            const line = [...view.contentDOM.querySelectorAll(".cm-line")]
                .find(element => element.textContent.includes(substring));
            return line ? line.className : null;
        },
        select(substring, occurrence = 0, collapseToEnd = false) {
            const text = view.state.doc.toString();
            let index = -1;
            for (let n = 0; n <= occurrence; n++) index = text.indexOf(substring, index + 1);
            if (index < 0) throw new Error("Missing text: " + substring);
            const anchor = collapseToEnd ? index + substring.length : index;
            view.dispatch({ selection: { anchor, head: index + substring.length } });
            view.focus();
        },
        moveTo(offset) {
            view.dispatch({ selection: { anchor: offset } });
        },
        showProposal(start, end, replacement) {
            const from = utf16Offset(view.state, start);
            const to = utf16Offset(view.state, end);
            view.dispatch({ effects: setProposal.of({
                from, to, replacement, removed: view.state.sliceDoc(from, to), stale: false,
            }) });
            review.replaceChildren();
            const status = document.createElement("p");
            status.setAttribute("role", "status");
            status.textContent = "Pending proposal";
            review.append(status);
            for (const [name, action] of [["Accept proposal", "accept"], ["Reject proposal", "reject"]]) {
                const button = document.createElement("button");
                button.textContent = name;
                button.addEventListener("click", () => {
                    const proposal = view.state.field(proposalField);
                    if (!proposal) return;
                    if (proposal.stale) {
                        status.textContent = "Proposal is stale. Request a fresh preview.";
                        window.reviewAction = "stale";
                        return;
                    }
                    if (action === "accept") {
                        // Remove the history field to clear both undo and redo at acceptance.
                        view.dispatch({
                            changes: { from: proposal.from, to: proposal.to, insert: proposal.replacement },
                            effects: [setProposal.of(null), historySlot.reconfigure([])],
                        });
                        view.dispatch({ effects: historySlot.reconfigure(history()) });
                    } else {
                        view.dispatch({ effects: setProposal.of(null) });
                    }
                    status.textContent = action === "accept" ? "Proposal accepted" : "Proposal rejected";
                    window.reviewAction = action;
                });
                review.append(button);
            }
        },
        proposalState() {
            const proposal = view.state.field(proposalField);
            return proposal ? (proposal.stale ? "stale" : "pending") : "none";
        },
    };
};

// "frame" is quantised by the display refresh. "work" ends after pending tasks and a forced
// layout, so it approximates main-thread cost before paint.
function measureInput(samples) {
    const start = performance.now();
    const channel = new MessageChannel();
    channel.port1.onmessage = () => {
        void document.body.offsetHeight;
        samples.work.push(performance.now() - start);
    };
    channel.port2.postMessage(null);
    requestAnimationFrame(() => requestAnimationFrame(() => {
        samples.frame.push(performance.now() - start);
    }));
}

window.mountScaleFixture = () => {
    const paragraph = Array.from({ length: 20 }, () => "word").join(" ");
    const body = Array.from({ length: 2500 }, () => paragraph).join("\n\n");
    const { view } = createView(document.querySelector("main"), body, { label: "Scale fixture" });
    const samples = { frame: [], work: [] };
    view.contentDOM.addEventListener("beforeinput", () => measureInput(samples));
    view.dispatch({ selection: { anchor: Math.floor(body.length / 2) }, scrollIntoView: true });
    return samples;
};
