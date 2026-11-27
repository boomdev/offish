import assert from "node:assert/strict";
import test from "node:test";
import {remapOpenDocuments} from "../explorer.mjs";

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
