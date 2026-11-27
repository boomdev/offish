import assert from "node:assert/strict";
import test from "node:test";
import {ancestorFolders, paneDestination} from "../pane-focus.mjs";

test("horizontal navigation treats both resize dividers as separate stops", () => {
    const order = ["files", "files-divider", "editor", "details-divider", "details"];
    for (let index = 0; index < order.length; index++) {
        assert.equal(paneDestination(order[index], "editor", "ArrowLeft"), order[index - 1] ?? null);
        assert.equal(paneDestination(order[index], "editor", "ArrowRight"), order[index + 1] ?? null);
    }
});

test("menu navigation returns to the previous workspace stop", () => {
    assert.equal(paneDestination("details-divider", "editor", "ArrowUp"), "menu");
    assert.equal(paneDestination("menu", "details-divider", "ArrowDown"), "details-divider");
    assert.equal(paneDestination("editor", "details-divider", "ArrowDown"), null);
});

test("revealing an open document expands every containing folder", () => {
    assert.deepEqual(ancestorFolders("Notes/pilot/brief.md"), ["Notes", "Notes/pilot"]);
    assert.deepEqual(ancestorFolders("brief.md"), []);
});
