import assert from "node:assert/strict";
import test from "node:test";
import {menuPosition} from "../menu.mjs";

test("a floating menu opens at the trigger and stays inside the viewport edges", () => {
    const viewport = {width: 700, height: 420};
    const size = {width: 160, height: 120};
    assert.deepEqual(menuPosition({left: 20, bottom: 40}, size, viewport), {left: 20, top: 40});
    assert.deepEqual(menuPosition({left: 680, bottom: 413}, size, viewport), {left: 534, top: 294});
});
