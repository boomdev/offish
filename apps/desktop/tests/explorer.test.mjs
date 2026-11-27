import assert from "node:assert/strict";
import test from "node:test";
import {Text} from "@codemirror/state";
import {displayPath, remapOpenDocuments, searchSelection, treeCommand, workspaceFilePath} from "../explorer.mjs";

test("search jumps use UTF-16 editor offsets and validate the actual open text", () => {
    const hit = {line: 2, column: 3, matched: "NÉEDLE"};
    const doc = Text.of(["first", "😀 NÉEDLE more"]);
    assert.deepEqual(searchSelection(doc, hit), {anchor: 9, head: 15});
    assert.deepEqual(searchSelection(Text.of(["first", "draft text"]), hit), {anchor: 6});
    assert.deepEqual(searchSelection(Text.of(["draft only"]), {...hit, line: 10}), {anchor: 0});
});

test("root labels are shown without exposing routing IDs", () => {
    const roots = [{name: "root-0", label: "Notes"}, {name: "root-1", label: "Project"}];
    assert.equal(displayPath("root-0/brief.md", roots), "Notes/brief.md");
    assert.equal(displayPath("root-1/brief.md", roots), "Project/brief.md");
    assert.equal(displayPath("root-1", roots), "Project");
});

test("overlapping roots resolve both tab paths to the same file", () => {
    const roots = [{name: "root-0", hint: "/"}, {name: "root-1", hint: "/documents"}];
    assert.equal(workspaceFilePath("root-0/documents/notes.md", roots), "/documents/notes.md");
    assert.equal(workspaceFilePath("root-1/notes.md", roots), "/documents/notes.md");
    assert.equal(workspaceFilePath("root-1", roots), "/documents");
});

test("workspace roots can expand and collapse but cannot be renamed or deleted", () => {
    const rows = [{path: "root-0", root: true, expanded: false}, {path: "root-1", root: true, expanded: true}];
    assert.deepEqual(treeCommand(rows, 0, "ArrowRight"), {kind: "activate"});
    assert.deepEqual(treeCommand(rows, 0, "ArrowDown"), {kind: "focus", path: "root-1"});
    assert.deepEqual(treeCommand(rows, 1, "ArrowLeft"), {kind: "activate"});
    for (const key of ["Delete", "F2", "ContextMenu"]) {
        assert.equal(treeCommand(rows, 0, key), null);
    }
});

test("Enter on a focused folder action is left to the button", () => {
    const rows = [{path: "root-0/Notes", expanded: false}];
    assert.equal(treeCommand(rows, 0, "Enter", false, true), null);
    assert.deepEqual(treeCommand(rows, 0, "Enter"), {kind: "activate"});
});

test("folder rename retains the very same live drafts for descendants", () => {
    const draft = {text: "pending", base: {revision: "abc"}};
    const other = {text: "unchanged"};
    const docs = new Map([["Notes/sub/file.md", draft], ["Notes-old/file.md", other]]);
    const next = new Map(remapOpenDocuments(docs, "Notes", "Archive"));
    assert.equal(next.get("Archive/sub/file.md"), draft);
    assert.equal(next.get("Notes-old/file.md"), other);
    assert.equal(docs.get("Notes/sub/file.md"), draft);
});

test("destination tab collision must be resolved before a file rename", () => {
    const docs = new Map([["original.md", {}], ["existing.md", {}]]);
    assert.throws(() => remapOpenDocuments(docs, "original.md", "existing.md"), /Close the tab/);
});

test("moves keep the same draft object across parent folders", () => {
    const draft = {text: "pending", base: {revision: "abc"}};
    const docs = new Map([["Notes/brief.md", draft]]);
    const destination = new Map(remapOpenDocuments(docs, "Notes/brief.md", "Archive/brief.md"));
    assert.equal(destination.get("Archive/brief.md"), draft);
    assert.ok(!destination.has("Notes/brief.md"));
});

test("tree arrows skip unsupported rows and follow visible hierarchy", () => {
    const rows = [
        {path: "Notes", expanded: true}, {path: "Notes/link", disabled: true},
        {path: "Notes/a.md"}, {path: "Archive", expanded: false},
    ];
    assert.deepEqual(treeCommand(rows, 0, "ArrowDown"), {kind: "focus", path: "Notes/a.md"});
    assert.deepEqual(treeCommand(rows, 2, "ArrowUp"), {kind: "focus", path: "Notes"});
    assert.deepEqual(treeCommand(rows, 0, "ArrowRight"), {kind: "focus", path: "Notes/a.md"});
    assert.deepEqual(treeCommand(rows, 2, "ArrowLeft"), {kind: "focus", path: "Notes"});
    assert.deepEqual(treeCommand(rows, 0, "ArrowLeft"), {kind: "activate"});
    assert.deepEqual(treeCommand(rows, 3, "ArrowRight"), {kind: "activate"});
});

test("editing and deletion keys dispatch distinct tree actions", () => {
    const rows = [{path: "draft.md"}];
    for (const [key, shift, kind] of [
        ["Enter", false, "activate"], ["F2", false, "rename"],
        ["Delete", false, "trash"], ["Delete", true, "delete"], ["F10", true, "menu"],
    ]) {
        assert.deepEqual(treeCommand(rows, 0, key, shift), {kind});
    }
});
