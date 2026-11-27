import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import test from "node:test";
import {syntaxTree} from "@codemirror/language";
import {Compartment, EditorState} from "@codemirror/state";
import {EditorView} from "@codemirror/view";
import {inlineDecorations, markdownPreview, previewLanguage} from "../markdown-preview.mjs";
import {sourceEditBatch} from "../source-edits.mjs";

const brief = readFileSync(new URL("../../../plans/research/phase-0/fixtures/pilot/brief.md", import.meta.url), "utf8");

function decorations(state, load = () => {}) {
    const result = [];
    inlineDecorations(state, [{from: 0, to: state.doc.length}], load)
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
