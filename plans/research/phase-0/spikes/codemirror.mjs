import { history, historyKeymap, insertNewline } from "@codemirror/commands";
import { markdown, commonmarkLanguage } from "@codemirror/lang-markdown";
import { yamlFrontmatter } from "@codemirror/lang-yaml";
import { syntaxTree, ensureSyntaxTree } from "@codemirror/language";
import { EditorState, StateEffect, StateField, RangeSetBuilder, Transaction } from "@codemirror/state";
import { Decoration, EditorView, ViewPlugin, WidgetType, keymap } from "@codemirror/view";
import { Table } from "@lezer/markdown";

// Offish Markdown profile candidate: CommonMark, pipe tables and YAML front matter.
const language = yamlFrontmatter({ content: markdown({ base: commonmarkLanguage, extensions: [Table] }) });
const encoder = new TextEncoder();
const assetBase = "https://offish.invalid/";

// Editor positions count each line break as one unit, whatever its bytes. Byte offsets
// re-encode the prefix with the file's line break; production code needs a line index.
const utf8Offset = (state, doc, offset) =>
    encoder.encode(doc.sliceString(0, offset, state.lineBreak)).length;

function utf16Offset(state, bytes) {
    const { doc } = state;
    let low = 0, high = doc.length;
    while (low < high) {
        const middle = (low + high) >> 1;
        if (utf8Offset(state, doc, middle) < bytes) low = middle + 1; else high = middle;
    }
    if (utf8Offset(state, doc, low) !== bytes) throw new Error("Byte offset is not on a character boundary");
    return low;
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
        image.src = new URL(this.src, assetBase).href;
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

// Block widgets that replace line breaks must come from state.
const tablePreview = StateField.define({
    create: state => buildTables(state),
    update(value, transaction) {
        if (transaction.docChanged || transaction.selection ||
            syntaxTree(transaction.startState) !== syntaxTree(transaction.state)) {
            return buildTables(transaction.state);
        }
        return value;
    },
    provide: field => EditorView.decorations.from(field),
});

function buildTables(state) {
    const builder = new RangeSetBuilder();
    // Front matter support mounts the Markdown tree inside a wrapper, so walk the whole tree.
    syntaxTree(state).iterate({ enter: node => {
        if (node.name !== "Table") return !["Paragraph", "Frontmatter", "CommentBlock"].includes(node.name);
        if (!touchesSelection(state, node.from, node.to)) {
            builder.add(node.from, node.to, Decoration.replace({
                widget: new TableWidget(tableRows(state, node.node), node.from), block: true,
            }));
        }
        return false;
    } });
    return builder.finish();
}

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
    "del": { background: "#fdd", textDecoration: "line-through" },
    "ins": { background: "#dfd", textDecoration: "underline" },
    ".cm-sr-only": { position: "absolute", width: "1px", height: "1px", overflow: "hidden", clip: "rect(0 0 0 0)" },
    ".cm-table-preview td, .cm-table-preview th": { border: "1px solid #999", padding: "2px 8px" },
});

function createView(parent, source, { label, separator, preview = true }) {
    const operations = [];
    const extensions = [
        language, theme, proposalField, normaliseInput, history(),
        keymap.of([{ key: "Enter", run: insertNewline }, ...historyKeymap]),
        EditorView.contentAttributes.of({ "aria-label": label }),
        EditorView.updateListener.of(update => {
            if (!update.docChanged) return;
            // Typed operations against the previous revision, in UTF-8 bytes.
            update.changes.iterChanges((fromA, toA, _fromB, _toB, inserted) => {
                operations.push({
                    start: utf8Offset(update.startState, update.startState.doc, fromA),
                    end: utf8Offset(update.startState, update.startState.doc, toA),
                    replacement: inserted.sliceString(0, inserted.length, update.state.lineBreak),
                });
            });
        }),
    ];
    if (preview) extensions.push(inlinePreview, tablePreview);
    if (separator !== undefined) extensions.push(EditorState.lineSeparator.of(separator));
    const view = new EditorView({ parent, state: EditorState.create({ doc: source, extensions }) });
    return { view, operations };
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
    const { view, operations } = createView(main, source, {
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
                    const changes = action === "accept"
                        ? { from: proposal.from, to: proposal.to, insert: proposal.replacement } : [];
                    view.dispatch({ changes, effects: setProposal.of(null) });
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
