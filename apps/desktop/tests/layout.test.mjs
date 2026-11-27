import assert from "node:assert/strict";
import test from "node:test";
import {fitPanes, paneWidth} from "../layout.mjs";

test("either divider clamps to the pane minimum and preserves the editor", () => {
    assert.deepEqual(paneWidth(900, "files", 204, 800), {min: 145, max: 414, width: 414});
    assert.deepEqual(paneWidth(900, "details", 190, 100), {min: 165, max: 428, width: 165});
});

test("narrowing the window leaves both panes and the editor usable", () => {
    const {files, details} = fitPanes(700, 390, 350);
    assert.ok(files >= 145 && details >= 165);
    assert.ok(700 - files - details - 2 >= 280);
});
