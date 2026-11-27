import assert from "node:assert/strict";
import test from "node:test";
import {EditorState} from "@codemirror/state";

test("CodeMirror reports the UTF-16 and CRLF positions used by the Rust source contract", () => {
    const source = "\uFEFFA😀\r\n日本語\r\n";
    const state = EditorState.create({
        doc: source.slice(1),
        extensions: [EditorState.lineSeparator.of("\r\n")],
    });
    assert.equal(state.doc.length, 8);
    assert.equal(state.sliceDoc(1, 3), "😀");
    assert.equal(state.sliceDoc(3, 4), "\r\n");
    assert.equal(state.sliceDoc(4, 7), "日本語");
    assert.equal(state.sliceDoc(), source.slice(1));
});
