import assert from "node:assert/strict";
import { test } from "node:test";
import { setupConversation } from "../conversation.mjs";

function fixtureDOM(t) {
    const nodes = new Map();
    function element() {
        const handlers = new Map();
        return {
            open: false, hidden: false, disabled: false, value: "", children: [],
            setAttribute() {},
            focus() { document.activeElement = this; },
            append(...children) { this.children.push(...children); },
            querySelectorAll() { return []; },
            replaceChildren(...children) { this.children = children; },
            addEventListener(type, action) { handlers.set(type, action); },
            showModal() { this.open = true; },
            close() {
                this.open = false;
                return handlers.get("close")?.();
            },
        };
    }
    const original = globalThis.document;
    globalThis.document = {
        createElement: element,
        getElementById(id) {
            if (!nodes.has(id)) nodes.set(id, element());
            return nodes.get(id);
        },
    };
    t.after(() => { globalThis.document = original; });
    return id => document.getElementById(id);
}

const setting = {
    labelForPath: path => path,
    getSelection: () => ({path: "root-0/brief.md", base: "revision", from: 0, to: 5}),
    reviewProposal() { assert.fail("A cancelled turn cannot launch review."); },
};

test("approval is separate from preparation and declining starts no provider", async t => {
    const $ = fixtureDOM(t);
    const invoke = t.mock.fn(async (command, args) => {
        if (command === "conversation_history") return [];
        if (command === "prepare_agent_turn") {
            assert.equal(args.request.prompt, "Please revise.");
            return {
                id: "turn", prompt: "Please revise.",
                selection: {path: "root-0/brief.md", text: "<private>"},
                files: [{path: "root-0/ref.txt", text: "Reference excerpt."}],
            };
        }
        assert.equal(command, "cancel_agent_turn");
        assert.deepEqual(args, {id: "turn"});
    });
    await setupConversation({...setting, invoke})(true);
    $("conversation-prompt").value = "Please revise.";
    await $("conversation-form").onsubmit({preventDefault() {}});
    assert.equal($("context-dialog").open, true);
    assert.ok($("context-items").children.some(node => node.textContent === "<private>"));
    assert.deepEqual(invoke.mock.calls.map(call => call.arguments[0]),
        ["conversation_history", "prepare_agent_turn"]);
    await $("context-deny").onclick();
    assert.equal($("context-dialog").open, false);
    assert.equal($("context-items").children.length, 0);
    assert.equal($("conversation-history").hidden, true);
    assert.equal($("conversation-status").textContent, "Context was declined.");
});

test("a proposal arriving after cancellation cannot enter inline review", async t => {
    const $ = fixtureDOM(t);
    t.mock.timers.enable({apis: ["setTimeout"]});
    const message = {id: "turn", prompt: "Please revise.", answer: "", references: 0, outcome: "interrupted"};
    const late = Promise.withResolvers();
    const invoke = t.mock.fn(async command => {
        switch (command) {
            case "conversation_history": return [];
            case "prepare_agent_turn": return {
                id: message.id, prompt: message.prompt,
                selection: {path: "root-0/brief.md", text: "Hello"}, files: [],
            };
            case "approve_agent_turn": return {...message};
            case "poll_agent_turn": return late.promise;
            case "cancel_agent_turn": return {...message, outcome: "cancelled"};
            default: assert.fail(command);
        }
    });
    await setupConversation({...setting, invoke})(true);
    $("conversation-prompt").value = message.prompt;
    await $("conversation-form").onsubmit({preventDefault() {}});
    await $("context-approve").onclick();
    t.mock.timers.tick(1000);
    assert.equal(invoke.mock.calls.at(-1).arguments[0], "poll_agent_turn");
    await $("conversation-cancel").onclick();
    late.resolve({type: "done", message: {...message, outcome: "complete"}, staged: {path: "root-0/brief.md"}});
    await late.promise;
    await Promise.resolve();
    assert.equal($("conversation-review").hidden, true);
    assert.equal($("conversation-status").textContent, "Turn cancelled.");
});

test("Codex approval discloses the external destination and approves the prepared ID only", async t => {
    const $ = fixtureDOM(t);
    t.mock.timers.enable({apis: ["setTimeout"]});
    const invoke = t.mock.fn(async (command, args) => {
        switch (command) {
            case "conversation_history": return [];
            case "chatgpt_connected": return true;
            case "prepare_agent_turn":
                assert.equal(args.request.provider, "codex");
                return {
                    id: "codex-turn", provider: "codex", prompt: "Revise.",
                    selection: {path: "root-0/brief.md", text: "Approved excerpt."}, files: [],
                };
            case "approve_agent_turn":
                assert.deepEqual(args, {id: "codex-turn"});
                return {id: "codex-turn", provider: "codex", prompt: "Revise.", answer: "",
                    references: 0, outcome: "interrupted"};
            default: assert.fail(command);
        }
    });
    await setupConversation({...setting, invoke})(true);
    $("conversation-provider").value = "codex";
    await $("conversation-provider").onchange();
    $("conversation-prompt").value = "Revise.";
    await $("conversation-form").onsubmit({preventDefault() {}});
    assert.match($("context-intro").textContent, /to OpenAI using your ChatGPT plan/);
    assert.equal($("conversation-provider").disabled, true);
    assert.ok(!invoke.mock.calls.some(call => call.arguments[0] === "approve_agent_turn"));
    await $("context-approve").onclick();
    assert.equal(invoke.mock.calls.at(-1).arguments[0], "approve_agent_turn");
});
