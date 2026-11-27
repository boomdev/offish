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

export function inlineDecorations(state, visibleRanges, loadImage) {
    const ranges = [];
    const lines = new Set();
    const tree = syntaxTree(state);
    for (const {from, to} of visibleRanges) {
        tree.iterate({from, to, enter(node) {
            const line = state.doc.lineAt(node.from);
            const endLine = state.doc.lineAt(node.to);
            const editing = selected(state, line.from, endLine.to);
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
    constructor(rows) {
        super();
        this.rows = rows;
    }

    eq(other) { return JSON.stringify(this.rows) === JSON.stringify(other.rows); }

    toDOM(view) {
        const table = document.createElement("table");
        table.className = "cm-preview-table";
        for (const [index, cells] of this.rows.entries()) {
            const row = table.insertRow();
            for (const text of cells) {
                const cell = document.createElement(index === 0 ? "th" : "td");
                if (index === 0) cell.scope = "col";
                cell.textContent = text;
                row.append(cell);
            }
        }
        table.addEventListener("mousedown", event => {
            event.preventDefault();
            const anchor = view.posAtDOM(table);
            view.dispatch({selection: {anchor}});
            view.focus();
        });
        return table;
    }

    ignoreEvent() { return false; }
}

function parsedTables(state) {
    const tables = [];
    syntaxTree(state).iterate({enter(node) {
        if (node.name !== "Table") return !["Paragraph", "FrontMatter", "HTMLBlock", "CommentBlock"].includes(node.name);
        const rows = [];
        for (let row = node.node.firstChild; row; row = row.nextSibling) {
            if (row.name !== "TableHeader" && row.name !== "TableRow") continue;
            rows.push(row.getChildren("TableCell").map(cell => state.sliceDoc(cell.from, cell.to).trim()));
        }
        tables.push(Decoration.replace({block: true, widget: new PreviewTable(rows)}).range(node.from, node.to));
        return false;
    }});
    return tables;
}

function tableState(state, tables = parsedTables(state)) {
    const visible = tables.filter(({from, to}) => !selected(state, from, to));
    return {tables, decorations: Decoration.set(visible, true)};
}

const previewTables = StateField.define({
    create: tableState,
    update(value, transaction) {
        const treeChanged = syntaxTree(transaction.startState) !== syntaxTree(transaction.state);
        if (!transaction.docChanged && !treeChanged) {
            return transaction.selection ? tableState(transaction.state, value.tables) : value;
        }
        return tableState(transaction.state);
    },
    provide: field => EditorView.decorations.from(field, value => value.decorations),
});

export function markdownPreview(loadImage) {
    const inline = ViewPlugin.fromClass(class {
        constructor(view) { this.decorations = inlineDecorations(view.state, view.visibleRanges, loadImage); }
        update(update) {
            if (update.docChanged || update.selectionSet || update.viewportChanged ||
                syntaxTree(update.startState) !== syntaxTree(update.state)) {
                this.decorations = inlineDecorations(update.state, update.view.visibleRanges, loadImage);
            }
        }
    }, {decorations: plugin => plugin.decorations});
    return [previewLanguage, inline, previewTables];
}
