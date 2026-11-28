import { EditorState } from "@codemirror/state";
import { Decoration, EditorView, WidgetType } from "@codemirror/view";

const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", {fatal: true, ignoreBOM: true});

export function reviewHunks(state, source, proposal) {
    const bytes = encoder.encode(source);
    const bom = Number(source.startsWith("\uFEFF"));
    if (state.sliceDoc() !== source.slice(bom)) {
        throw new Error("The editor no longer matches the proposal source.");
    }
    return proposal.hunks.map(({id, edit}) => {
        const start = decoder.decode(bytes.subarray(0, edit.from)).length;
        const end = decoder.decode(bytes.subarray(0, edit.to)).length;
        if (source.slice(start, end) !== edit.expected || start < bom) {
            throw new Error("The proposal no longer matches the saved source.");
        }
        const from = state.toText(source.slice(bom, start)).length;
        const to = state.toText(source.slice(bom, end)).length;
        return {id, edit, from, to, anchor: state.doc.lineAt(to).to, start, end};
    });
}

class ReviewWidget extends WidgetType {
    constructor(hunk, busy, decide, next) {
        super();
        this.hunk = hunk;
        this.busy = busy;
        this.decide = decide;
        this.next = next;
    }

    toDOM() {
        const {id, edit: {expected, replacement}} = this.hunk;
        const block = document.createElement("div");
        block.className = "inline-review";
        block.setAttribute("role", "group");
        block.setAttribute("aria-label", `Proposed change to ${id}`);
        for (const [name, text] of [["Removal", expected], ["Addition", replacement]]) {
            if (!text) continue;
            const row = document.createElement("div");
            row.className = `review-${name.toLowerCase()}`;
            const label = document.createElement("strong");
            label.textContent = name;
            const content = document.createElement(name === "Removal" ? "del" : "ins");
            content.textContent = text;
            row.append(label, content);
            block.append(row);
        }
        const actions = document.createElement("div");
        actions.className = "review-actions";
        for (const action of ["Accept", "Reject"]) {
            const button = document.createElement("button");
            button.type = "button";
            button.textContent = action;
            button.setAttribute("aria-label", `${action} ${id} change`);
            button.disabled = this.busy;
            if (action === "Accept") button.dataset.reviewAccept = id;
            button.onclick = () => this.decide(action === "Accept", this.hunk);
            actions.append(button);
        }
        if (this.next) {
            const button = document.createElement("button");
            button.type = "button";
            button.textContent = "Next change";
            button.disabled = this.busy;
            button.onclick = this.next;
            actions.append(button);
        }
        block.append(actions);
        return block;
    }
}

export function inlineReview(hunks, busy, decide, jump) {
    const ranges = hunks.flatMap((hunk, index) => {
        const next = hunks.length > 1 ? () => jump((index + 1) % hunks.length) : null;
        const widget = Decoration.widget({
            widget: new ReviewWidget(hunk, busy, decide, next), block: true, side: 1,
        }).range(hunk.anchor);
        return hunk.from === hunk.to ? [widget] :
            [Decoration.mark({class: "review-source-removal"}).range(hunk.from, hunk.to), widget];
    });
    return [
        EditorState.readOnly.of(true), EditorView.editable.of(false),
        EditorView.decorations.of(Decoration.set(ranges, true)),
    ];
}
