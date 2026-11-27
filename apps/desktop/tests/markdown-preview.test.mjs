import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import test from "node:test";
import {syntaxTree} from "@codemirror/language";
import {Compartment, EditorState} from "@codemirror/state";
import {EditorView} from "@codemirror/view";
import {inlineDecorations, markdownPreview, previewLanguage} from "../markdown-preview.mjs";
import {sourceEditBatch} from "../source-edits.mjs";

const brief = readFileSync(new URL("../../../plans/research/phase-0/fixtures/pilot/brief.md", import.meta.url), "utf8");

function decorations(state, load = () => {}, simple = false) {
    const result = [];
    inlineDecorations(state, [{from: 0, to: state.doc.length}], load, simple)
        .between(0, state.doc.length, (from, to, value) => result.push({from, to, spec: value.spec}));
    return result;
}

test("preview parses closed metadata fences without interpreting their contents as Markdown", () => {
    for (const closer of ["---", "..."]) {
        const doc = "---\n\`\`\`\n" + closer + "\n# Heading\n";
        const state = EditorState.create({doc, extensions: previewLanguage});
        assert.equal(syntaxTree(state).topNode.firstChild.name, "FrontMatter");
        assert.equal(syntaxTree(state).topNode.lastChild.name, "ATXHeading1");
        const at = doc.indexOf(closer, 4);
        const open = state.update({changes: {from: at, to: at + 3}}).state;
        assert.equal(syntaxTree(open).topNode.firstChild.name, "HorizontalRule");
    }
});

test("the active line keeps its source markers and inactive headings, links and lists have decorations", () => {
    const doc = "# Heading\n\n* **bold** [link](note.md)\n  1. Nested\n";
    const extensions = markdownPreview(() => {});
    const state = EditorState.create({doc, extensions});
    const headingMark = ({from, to, spec}) => from === 0 && to === 1 && "widget" in spec;
    const initial = decorations(state);
    assert.equal(initial.some(headingMark), false);
    assert.ok(initial.some(({spec}) => spec.class?.includes("cm-preview-heading-1")));
    assert.ok(initial.some(({spec}) => spec.class === "cm-preview-strong"));
    assert.ok(initial.some(({spec}) => spec.class === "cm-preview-link"));
    const moved = state.update({selection: {anchor: doc.indexOf("Nested")}}).state;
    assert.ok(decorations(moved).some(headingMark));
    assert.equal(moved.sliceDoc(), doc);
});

test("table preview yields to source when the caret enters it", () => {
    const state = EditorState.create({doc: brief, extensions: markdownPreview(() => {})});
    const table = syntaxTree(state).topNode.getChild("Table");
    assert.ok(table);
    const ranges = state.facet(EditorView.decorations).filter(value => typeof value !== "function");
    assert.equal(ranges[0].size, 1);
    const inTable = state.update({selection: {anchor: table.from + 2}}).state;
    const restored = inTable.facet(EditorView.decorations).filter(value => typeof value !== "function");
    assert.equal(restored[0].size, 0);
    assert.equal(inTable.sliceDoc(), brief);
});

test("Simple mode hides supported markers on the active line and keeps tables rendered", () => {
    const state = EditorState.create({doc: brief, extensions: markdownPreview(() => {}, {simple: true})});
    const heading = syntaxTree(state).topNode.getChild("ATXHeading1");
    const editing = state.update({selection: {anchor: heading.from + 2}}).state;
    assert.ok(decorations(editing, undefined, true)
        .some(({from, to, spec}) => from === heading.from && to === heading.from + 1 && "widget" in spec));
    const table = syntaxTree(state).topNode.getChild("Table");
    const inTable = state.update({selection: {anchor: table.from + 2}}).state;
    const ranges = inTable.facet(EditorView.decorations).filter(value => typeof value !== "function");
    assert.equal(ranges[0].size, 1);
    assert.equal(inTable.sliceDoc(), brief);
});

test("a Simple table cell opens as a keyboard-editable input with its exact table address", async t => {
    function element(tagName) {
        return {
            tagName: tagName.toUpperCase(), classList: {add() {}}, children: [], dataset: {}, isConnected: true,
            append(child) { child.parentElement = this; this.children.push(child); },
            replaceChildren(child) { this.children = []; this.append(child); },
            insertRow() { const row = element("tr"); this.append(row); return row; },
            addEventListener() {},
            focus() { document.activeElement = this; },
        };
    }
    const original = globalThis.document;
    globalThis.document = {createElement: element};
    t.after(() => { globalThis.document = original; });
    const draft = {value: ""};
    const actions = {
        current: () => null,
        report: assert.fail,
        begin: t.mock.fn(async address => ({...draft, address})),
        change: (draft, value) => { draft.value = value; },
        apply: t.mock.fn(async () => {}),
        cancel: t.mock.fn(draft => draft.onFinish()),
    };
    const doc = brief.replace("| Budget (USD) | 2,500 |", "| Budget (USD) |   |");
    const state = EditorState.create({doc, extensions: markdownPreview(() => {}, {simple: true, actions})});
    const table = syntaxTree(state).topNode.getChild("Table");
    const ranges = state.facet(EditorView.decorations).find(value => typeof value !== "function");
    const widget = ranges.iter().value.spec.widget;
    const view = {dom: {querySelector: t.mock.fn()}};
    const cell = widget.toDOM(view).children[2].children[1];
    const button = cell.children[0];
    await button.onclick();
    assert.deepEqual(actions.begin.mock.calls[0].arguments, [
        {table_from: table.from, table_to: table.to, row: 2, column: 1},
    ]);
    const [input] = cell.children;
    assert.equal(document.activeElement, input);
    assert.equal(input.value, "");
    assert.equal(input.rows, 1);
    input.onkeydown({key: "Enter", shiftKey: true, preventDefault: assert.fail});
    assert.equal(actions.apply.mock.callCount(), 0);
    // The textarea's native input event follows Shift+Enter.
    input.value = "2,600\n\nconfirmed";
    input.oninput();
    assert.equal(input.rows, 3);
    input.onkeydown({key: "Enter", isComposing: true, preventDefault: assert.fail});
    assert.equal(actions.apply.mock.callCount(), 0);
    input.onkeydown({key: "Enter", preventDefault() {}});
    assert.equal(actions.apply.mock.calls[0].arguments[0].value, "2,600\n\nconfirmed");
    input.onkeydown({key: "Escape", preventDefault() {}});
    assert.equal(cell.children[0], button);
    assert.equal(document.activeElement, button);

    const updated = state.update({changes: {from: doc.indexOf("Pilot seats"), insert: "Current "}}).state;
    const newTable = syntaxTree(updated).topNode.getChild("Table");
    const newWidget = updated.facet(EditorView.decorations).find(value => typeof value !== "function")
        .iter().value.spec.widget;
    const newCell = newWidget.toDOM(view).children[2].children[1];
    actions.begin.mock.mockImplementationOnce(async address => {
        button.isConnected = false;
        return {...draft, address: {...address, table_to: newTable.to}};
    });
    view.dom.querySelector.mock.mockImplementation(() => newCell.children[0]);
    await button.onclick();
    assert.deepEqual(view.dom.querySelector.mock.calls[0].arguments, [
        `[data-table-cell="${newTable.from}:${newTable.to}:2:1"]`,
    ]);
    assert.equal(document.activeElement, newCell.children[0]);
    assert.equal(newCell.children[0].tagName, "TEXTAREA");
});

test("table previews display consecutive breaks and preserve literal tags in inline code", t => {
    function element(tagName) {
        return {
            tagName, children: [],
            append(child) { this.children.push(child); },
            insertRow() { const row = element("tr"); this.append(row); return row; },
            addEventListener() {},
        };
    }
    const original = globalThis.document;
    globalThis.document = {createElement: element};
    t.after(() => { globalThis.document = original; });
    const doc = "\n| A | B |\n| --- | --- |\n| first<br><br>third | `<br>` |\n";
    const state = EditorState.create({doc, extensions: markdownPreview(() => {})});
    const ranges = state.facet(EditorView.decorations).find(value => typeof value !== "function");
    const [value, code] = ranges.iter().value.spec.widget.toDOM().children[1].children;
    assert.equal(value.textContent, "first\n\nthird");
    assert.equal(code.textContent, "`<br>`");
    assert.equal(state.sliceDoc(), doc);
});

test("table previews skip the native context menu outside the active cell editor", t => {
    const original = globalThis.document;
    globalThis.document = {createElement() {
        return {
            handlers: {},
            append() {},
            insertRow() { return {append() {}}; },
            addEventListener(name, handler) { this.handlers[name] = handler; },
        };
    }};
    t.after(() => { globalThis.document = original; });
    const state = EditorState.create({doc: brief, extensions: markdownPreview(() => {})});
    const ranges = state.facet(EditorView.decorations).find(value => typeof value !== "function");
    const view = {posAtDOM: () => 4, dispatch: t.mock.fn(), focus: t.mock.fn()};
    const table = ranges.iter().value.spec.widget.toDOM(view);
    const preventDefault = t.mock.fn();
    table.handlers.mousedown({button: 2, preventDefault});
    assert.equal(view.dispatch.mock.callCount(), 0);
    assert.equal(preventDefault.mock.callCount(), 0);
    table.handlers.contextmenu({target: {closest: () => null}, preventDefault});
    assert.equal(preventDefault.mock.callCount(), 1);
    table.handlers.contextmenu({target: {closest: () => ({})}, preventDefault});
    assert.equal(preventDefault.mock.callCount(), 1);
    table.handlers.mousedown({button: 0, preventDefault});
    assert.deepEqual(view.dispatch.mock.calls[0].arguments, [{selection: {anchor: 4}}]);
});

test("the pilot image creates a scoped loader widget and reveals source on its line", () => {
    const state = EditorState.create({doc: brief, extensions: previewLanguage});
    const match = decorations(state).find(({spec}) => spec.widget?.alt === "Pilot timeline");
    assert.equal(match?.spec.widget.path, "assets/timeline.svg");
    const imageLine = state.doc.lineAt(match.from);
    const editing = state.update({selection: {anchor: imageLine.from}}).state;
    assert.ok(!decorations(editing).some(({spec}) => spec.widget?.alt === "Pilot timeline"));
});

test("the image widget creates a Blob from workspace bytes and releases its URL", async t => {
    const createElement = tagName => ({
        tagName, dataset: {}, isConnected: true,
        replaceChildren(...children) { this.children = children; },
    });
    const original = globalThis.document;
    globalThis.document = {createElement};
    t.after(() => { globalThis.document = original; });
    const state = EditorState.create({doc: brief, extensions: previewLanguage});
    const load = t.mock.fn(async () => ({mime: "image/svg+xml", bytes: [60, 115, 118, 103, 47, 62]}));
    const match = decorations(state, load).find(({spec}) => spec.widget?.alt === "Pilot timeline");
    const element = match.spec.widget.toDOM();
    await new Promise(resolve => setImmediate(resolve));
    assert.deepEqual(load.mock.calls[0].arguments, ["assets/timeline.svg"]);
    assert.equal(element.children[0].tagName, "img");
    assert.equal(element.children[0].alt, "Pilot timeline");
    assert.match(element.children[0].src, /^blob:/);
    match.spec.widget.destroy(element);
    assert.equal(element.dataset.imageUrl, undefined);
});

test("source and preview can share the same edited state and selection", () => {
    const mode = new Compartment();
    const extensions = markdownPreview(() => {});
    const state = EditorState.create({doc: brief, extensions: mode.of(extensions)});
    const edited = state.update({changes: {from: brief.indexOf("Pilot launch"), insert: "New "}}).state;
    const source = edited.update({effects: mode.reconfigure([]), selection: {anchor: 18}}).state;
    assert.equal(syntaxTree(source).length, 0);
    const preview = source.update({effects: mode.reconfigure(extensions)}).state;
    assert.equal(preview.doc, edited.doc);
    assert.equal(preview.selection.main.anchor, 18);
    assert.ok(syntaxTree(preview).topNode.getChild("ATXHeading1"));
});

test("save splice uses file byte offsets and keeps BOM, CRLF and adjacent Unicode intact", () => {
    const source = "\uFEFFA😀\r\nZ";
    const updated = "\uFEFFA😁\r\n日本語";
    const batch = sourceEditBatch(source, updated);
    assert.deepEqual(batch, {
        profile: 1,
        edits: [{from: 4, to: 11, expected: "😀\r\nZ", replacement: "😁\r\n日本語"}],
    });
    const original = Buffer.from(source);
    const [{from, to, expected, replacement}] = batch.edits;
    assert.equal(original.subarray(from, to).toString(), expected);
    assert.equal(Buffer.concat([original.subarray(0, from), Buffer.from(replacement), original.subarray(to)]).toString(), updated);
    assert.deepEqual(sourceEditBatch(source, source), {profile: 1, edits: []});
});
