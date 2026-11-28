import assert from "node:assert/strict";
import { test } from "node:test";
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { inlineReview, reviewHunks } from "../inline-review.mjs";

function capturedProposal() {
    const prefix = "\uFEFFCafé 😀\r\n";
    const expected = "Budget";
    const source = prefix + expected + "\r\n";
    const from = Buffer.byteLength(prefix);
    const to = from + Buffer.byteLength(expected);
    const proposal = {hunks: [{id: "budget", edit: {from, to, expected, replacement: "<pilot>"}}]};
    const state = EditorState.create({
        doc: source.slice(1),
        extensions: EditorState.lineSeparator.of("\r\n"),
    });
    return {source, state, proposal};
}

test("review maps UTF-8 byte spans through BOM, Unicode and CRLF", () => {
    const {source, state, proposal} = capturedProposal();
    const [hunk] = reviewHunks(state, source, proposal);
    const line = state.doc.line(2);
    assert.deepEqual([hunk.from, hunk.to, hunk.anchor], [line.from, line.to, line.to]);
    assert.equal(source.slice(hunk.start, hunk.end), "Budget");
    proposal.hunks[0].edit.from++;
    assert.throws(() => reviewHunks(state, source, proposal), /no longer matches/);
});

test("inline review keeps source intact and exposes labelled native controls", t => {
    const {source, state, proposal} = capturedProposal();
    const hunks = reviewHunks(state, source, proposal);
    const decide = t.mock.fn();
    const old = globalThis.document;
    globalThis.document = {createElement: tag => ({
        tag,
        dataset: {},
        children: [],
        attributes: {},
        setAttribute(name, value) { this.attributes[name] = value; },
        append(...nodes) { this.children.push(...nodes); },
    })};
    t.after(() => { globalThis.document = old; });
    const review = EditorState.create({
        doc: state.doc,
        extensions: inlineReview(hunks, false, decide),
    });
    const [ranges] = review.facet(EditorView.decorations);
    const cursor = ranges.iter();
    assert.equal(cursor.value.spec.class, "review-source-removal");
    cursor.next();
    const widget = cursor.value.spec.widget;
    const group = widget.toDOM();
    const [removal, addition, actions] = group.children;
    assert.equal(group.attributes["aria-label"], "Proposed change to budget");
    assert.deepEqual(removal.children.map(node => node.textContent), ["Removal", "Budget"]);
    assert.deepEqual(addition.children.map(node => node.textContent), ["Addition", "<pilot>"]);
    assert.deepEqual(actions.children.map(node => node.attributes["aria-label"]),
        ["Accept budget change", "Reject budget change"]);
    actions.children[1].onclick();
    actions.children[0].onclick();
    assert.deepEqual(decide.mock.calls.map(call => call.arguments), [[false, hunks[0]], [true, hunks[0]]]);
    assert.equal(review.readOnly, true);
    assert.equal(review.doc.toString(), state.doc.toString());
});
