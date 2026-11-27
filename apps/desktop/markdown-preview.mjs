import {Language, defineLanguageFacet, languageDataProp, syntaxTree} from "@codemirror/language";
import {StateField} from "@codemirror/state";
import {Decoration, EditorView, ViewPlugin, WidgetType} from "@codemirror/view";
import {Table, parser} from "@lezer/markdown";

const isClosingFence = line => line === "---" || line === "...";
const languageData = defineLanguageFacet();
const markdown = parser.configure([Table, {props: [languageDataProp.add(type => type.isTop ? languageData : undefined)]}]);
const frontMatterParser = markdown.configure({
    defineNodes: [{name: "FrontMatter", block: true}],
    parseBlock: [{
        name: "FrontMatter",
        before: "HorizontalRule",
        parse(context, line) {
            if (context.lineStart !== 0 || line.text !== "---") return false;
            while (context.nextLine()) {
                if (!isClosingFence(line.text)) continue;
                const end = context.lineStart + line.text.length;
                context.nextLine();
                context.addElement(context.elt("FrontMatter", 0, end));
                return true;
            }
            context.addElement(context.elt("FrontMatter", 0, context.prevLineEnd()));
            return true;
        },
    }],
});

function hasFrontMatter(input) {
    if (input.length < 7 || input.read(0, 4) !== "---\n") return false;
    if (!input.lineChunks) return /(?:^|\n)(?:---|\.\.\.)(?=\n|$)/.test(input.read(4, input.length));
    for (let offset = 4; offset < input.length;) {
        const line = input.chunk(offset);
        if (isClosingFence(line)) return true;
        if (!line.length) return false;
        offset += line.length;
    }
    return false;
}

export const previewLanguage = new Language(languageData, markdown.configure({
    // An unclosed opening fence remains ordinary Markdown.
    wrap: (parse, input, fragments, ranges) => hasFrontMatter(input) ?
        frontMatterParser.startParse(input, fragments, ranges) : parse,
}), [], "Markdown preview");

const selected = (state, from, to) =>
    state.selection.ranges.some(range => range.from <= to && range.to >= from);
const mark = className => Decoration.mark({class: className});
const lineStyle = className => Decoration.line({class: className});

class Bullet extends WidgetType {
    eq() { return true; }
    toDOM() {
        const bullet = document.createElement("span");
        bullet.textContent = "• ";
        return bullet;
    }
}

class PreviewImage extends WidgetType {
    constructor(alt, path, load) {
        super();
        this.alt = alt;
        this.path = path;
        this.load = load;
    }

    eq(other) { return this.alt === other.alt && this.path === other.path; }

    toDOM() {
        const wrapper = document.createElement("span");
        wrapper.className = "cm-preview-image";
        wrapper.textContent = this.alt;
        const fallback = () => { wrapper.textContent = this.alt || "Image unavailable"; };
        Promise.resolve().then(() => this.load(decodeURIComponent(this.path))).then(({mime, bytes}) => {
            if (!wrapper.isConnected) return;
            const image = document.createElement("img");
            image.alt = this.alt;
            image.draggable = false;
            image.onload = () => releaseImage(wrapper);
            image.onerror = () => { releaseImage(wrapper); fallback(); };
            wrapper.dataset.imageUrl = URL.createObjectURL(new Blob([Uint8Array.from(bytes)], {type: mime}));
            image.src = wrapper.dataset.imageUrl;
            wrapper.replaceChildren(image);
        }).catch(fallback);
        return wrapper;
    }

    destroy(wrapper) { releaseImage(wrapper); }
}

function releaseImage(wrapper) {
    if (wrapper.dataset.imageUrl) {
        URL.revokeObjectURL(wrapper.dataset.imageUrl);
        delete wrapper.dataset.imageUrl;
    }
}

function hide(range, state, node, widget) {
    const start = node.from;
    const end = widget && state.sliceDoc(node.to, node.to + 1) === " " ? node.to + 1 : node.to;
    range.push(Decoration.replace({widget}).range(start, end));
}

export function inlineDecorations(state, visibleRanges, loadImage, simple = false) {
    const ranges = [];
    const lines = new Set();
    const tree = syntaxTree(state);
    for (const {from, to} of visibleRanges) {
        tree.iterate({from, to, enter(node) {
            const line = state.doc.lineAt(node.from);
            const endLine = state.doc.lineAt(node.to);
            const editing = !simple && selected(state, line.from, endLine.to);
            const heading = /^(?:ATX|Setext)Heading([1-6])$/.exec(node.name);
            const styleLine = name => {
                const key = line.from + ":" + name;
                if (lines.has(key)) return;
                lines.add(key);
                ranges.push(lineStyle(name).range(line.from));
            };
            if (heading) {
                styleLine("cm-preview-heading cm-preview-heading-" + heading[1]);
                return true;
            }
            switch (node.name) {
                case "Table":
                    return false;
                case "FrontMatter": case "CommentBlock": case "Comment": case "HTMLBlock": case "HTMLTag":
                    ranges.push(mark("cm-preview-opaque").range(node.from, node.to));
                    return false;
                case "StrongEmphasis": ranges.push(mark("cm-preview-strong").range(node.from, node.to)); break;
                case "Emphasis": ranges.push(mark("cm-preview-emphasis").range(node.from, node.to)); break;
                case "InlineCode": ranges.push(mark("cm-preview-code").range(node.from, node.to)); break;
                case "Link": ranges.push(mark("cm-preview-link").range(node.from, node.to)); break;
                case "QuoteMark":
                    styleLine("cm-preview-quote");
                    if (!editing) hide(ranges, state, node);
                    return false;
                case "HeaderMark": case "EmphasisMark": case "CodeMark":
                    if (!editing) hide(ranges, state, node);
                    return false;
                case "LinkMark": case "URL": case "LinkTitle": {
                    const parent = node.node.parent;
                    if (!editing && parent?.name === "Link" && parent.getChild("URL")) hide(ranges, state, node);
                    return false;
                }
                case "ListMark":
                    if (!editing && /^[*+-]$/.test(state.sliceDoc(node.from, node.to))) {
                        hide(ranges, state, node, new Bullet());
                    }
                    return false;
                case "Image": {
                    const url = node.node.getChild("URL");
                    const [open, close] = node.node.getChildren("LinkMark");
                    if (!editing && url && open && close) {
                        const alt = state.sliceDoc(open.to, close.from);
                        const path = state.sliceDoc(url.from, url.to).replace(/^<(.*)>$/, "$1");
                        const widget = new PreviewImage(alt, path, loadImage);
                        ranges.push(Decoration.replace({widget}).range(node.from, node.to));
                    }
                    return false;
                }
            }
            return true;
        }});
    }
    return Decoration.set(ranges, true);
}

class PreviewTable extends WidgetType {
    constructor(rows, from, to, actions) {
        super();
        this.rows = rows;
        this.from = from;
        this.to = to;
        this.actions = actions;
    }

    eq(other) {
        return this.actions === other.actions &&
            (!this.actions || this.from === other.from && this.to === other.to) &&
            JSON.stringify(this.rows) === JSON.stringify(other.rows);
    }

    toDOM(view) {
        const table = document.createElement("table");
        table.className = "cm-preview-table";
        if (this.actions) table.classList.add("cm-editable-table");
        for (const [index, cells] of this.rows.entries()) {
            const row = table.insertRow();
            for (const [column, text] of cells.entries()) {
                const cell = document.createElement(index === 0 ? "th" : "td");
                if (index === 0) cell.scope = "col";
                if (this.actions) {
                    editableCell(cell, text, {
                        table_from: this.from, table_to: this.to, row: index, column,
                    }, this.actions, view);
                } else {
                    cell.textContent = text;
                }
                row.append(cell);
            }
        }
        table.addEventListener("contextmenu", event => {
            if (!event.target.closest?.("textarea")) event.preventDefault();
        });
        if (!this.actions) table.addEventListener("mousedown", event => {
            if (event.button !== 0) return;
            event.preventDefault();
            const anchor = view.posAtDOM(table);
            view.dispatch({selection: {anchor}});
            view.focus();
        });
        return table;
    }

    ignoreEvent() { return Boolean(this.actions); }
}

const cellKey = ({table_from, table_to, row, column}) => `${table_from}:${table_to}:${row}:${column}`;
const cellLabel = ({row, column}) => `Row ${row + 1}, column ${column + 1}`;

function showCellInput(cell, button, draft, actions, focus) {
    const input = document.createElement("textarea");
    input.dataset.tableCell = button.dataset.tableCell;
    input.ariaLabel = cellLabel(draft.address);
    input.title = "Shift+Enter for a line break. Enter to apply. Escape to cancel.";
    input.value = draft.value;
    input.cols = 12;
    const fitRows = () => { input.rows = Math.min(8, input.value.split("\n").length); };
    fitRows();
    draft.focus = () => { if (input.isConnected) input.focus(); };
    draft.onFinish = () => {
        if (!input.isConnected) return;
        const focused = document.activeElement === input;
        cell.replaceChildren(button);
        if (focused) button.focus();
    };
    input.oninput = () => {
        fitRows();
        actions.change(draft, input.value);
    };
    input.onkeydown = event => {
        if (event.isComposing) return;
        if (event.key === "Escape") {
            event.preventDefault();
            actions.cancel(draft);
        } else if (event.key === "Enter" && !event.shiftKey) {
            event.preventDefault();
            actions.apply(draft).catch(actions.report);
        } else if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
            event.preventDefault();
            actions.save().catch(actions.report);
        }
    };
    cell.replaceChildren(input);
    if (focus) input.focus();
}

function editableCell(cell, text, address, actions, view) {
    const button = document.createElement("button");
    button.type = "button";
    button.dataset.tableCell = cellKey(address);
    button.textContent = text;
    button.ariaLabel = `${cellLabel(address)}: ${text.replaceAll("\n", " ") || "Empty"}`;
    button.onclick = async () => {
        try {
            const draft = await actions.begin(address);
            if (!draft) return;
            // Applying the last edit can replace the table widget and shift its address.
            const key = cellKey(draft.address);
            const target = button.isConnected && button.dataset.tableCell === key ? button :
                view.dom.querySelector(`[data-table-cell="${key}"]`);
            if (target?.tagName === "BUTTON") showCellInput(target.parentElement, target, draft, actions, true);
            else if (target) target.focus();
            else actions.cancel(draft);
        } catch (error) { actions.report(error); }
    };
    const draft = actions.current(address);
    if (draft) showCellInput(cell, button, draft, actions, false);
    else cell.append(button);
}

function displayCell(state, cell) {
    const lines = [];
    let from = cell.from;
    for (const tag of cell.getChildren("HTMLTag")) {
        if (state.sliceDoc(tag.from, tag.to) !== "<br>") continue;
        lines.push(state.sliceDoc(from, tag.from));
        from = tag.to;
    }
    lines.push(state.sliceDoc(from, cell.to));
    lines[0] = lines[0].trimStart();
    lines[lines.length - 1] = lines.at(-1).trimEnd();
    return lines.join("\n");
}

function parsedTables(state, actions) {
    const tables = [];
    syntaxTree(state).iterate({enter(node) {
        if (node.name !== "Table") return !["Paragraph", "FrontMatter", "HTMLBlock", "CommentBlock"].includes(node.name);
        const rows = [];
        for (let row = node.node.firstChild; row; row = row.nextSibling) {
            if (row.name !== "TableHeader" && row.name !== "TableRow") continue;
            // Lezer omits empty TableCell nodes. The parsed separators retain
            // their positions and distinguish structural pipes from escapes.
            const cells = [];
            const parsed = row.getChildren("TableCell");
            let nextCell = 0;
            let from = row.from;
            const addCell = to => {
                const node = parsed[nextCell];
                if (node && node.from >= from && node.to <= to) {
                    cells.push(displayCell(state, node));
                    nextCell++;
                } else {
                    cells.push(state.sliceDoc(from, to).trim());
                }
            };
            for (const pipe of row.getChildren("TableDelimiter")) {
                if (pipe.from !== row.from) addCell(pipe.from);
                from = pipe.to;
            }
            if (from < row.to) addCell(row.to);
            rows.push(cells);
        }
        tables.push(Decoration.replace({block: true, widget: new PreviewTable(rows, node.from, node.to, actions)}).range(node.from, node.to));
        return false;
    }});
    return tables;
}

function tablePreview(simple, actions) {
    function tableState(state, tables = parsedTables(state, actions)) {
        const visible = simple ? tables : tables.filter(({from, to}) => !selected(state, from, to));
        return {tables, decorations: Decoration.set(visible, true)};
    }
    return StateField.define({
        create: tableState,
        update(value, transaction) {
            const treeChanged = syntaxTree(transaction.startState) !== syntaxTree(transaction.state);
            if (!transaction.docChanged && !treeChanged) {
                return !simple && transaction.selection ? tableState(transaction.state, value.tables) : value;
            }
            return tableState(transaction.state);
        },
        provide: field => EditorView.decorations.from(field, value => value.decorations),
    });
}

export function markdownPreview(loadImage, {simple = false, actions} = {}) {
    const inline = ViewPlugin.fromClass(class {
        constructor(view) { this.decorations = inlineDecorations(view.state, view.visibleRanges, loadImage, simple); }
        update(update) {
            if (update.docChanged || update.selectionSet || update.viewportChanged ||
                syntaxTree(update.startState) !== syntaxTree(update.state)) {
                this.decorations = inlineDecorations(update.state, update.view.visibleRanges, loadImage, simple);
            }
        }
    }, {decorations: plugin => plugin.decorations});
    return [previewLanguage, inline, tablePreview(simple, actions)];
}
