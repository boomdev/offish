import MarkdownIt from "markdown-it";
import { defaultMarkdownParser, defaultMarkdownSerializer } from "prosemirror-markdown";
import { EditorState } from "prosemirror-state";
import { EditorView } from "prosemirror-view";

const selection = "Keep the first release on the current schedule.";
const markdown = new MarkdownIt("commonmark", { html: false }).enable("table");

window.exercise = (source) => {
    const frontMatter = /^---\n[\s\S]*?\n---\n/.exec(source)?.[0] ?? "";
    const body = source.slice(frontMatter.length);
    const preview = document.querySelector("article");
    preview.innerHTML = markdown.render(body.replace(/<!--[\s\S]*?-->/g, ""));

    // The parser's default schema exercises ordinary rich text. It lacks tables.
    const model = defaultMarkdownParser.parse(body);
    const firstParagraph = model.content.content.find(node => node.textContent === selection);
    if (!firstParagraph) throw new Error("Fixture selection is missing");
    const view = new EditorView(document.querySelector("main"), {
        state: EditorState.create({ doc: model.type.schema.nodes.doc.create(null, firstParagraph) })
    });
    view.dom.setAttribute("aria-label", "Selected paragraph");
    view.dom.setAttribute("role", "textbox");
    view.dom.setAttribute("aria-multiline", "true");

    return {
        defaultRoundTripPreservesBytes: defaultMarkdownSerializer.serialize(model) === body,
        defaultSchemaHasTable: Boolean(model.type.schema.nodes.table),
        savedSource() {
            // Deliberately narrow experiment. A generic editor needs structured source mapping.
            const lines = body.split("\n");
            const matches = markdown.parse(body, {}).filter(token =>
                token.type === "paragraph_open" &&
                token.map[1] === token.map[0] + 1 &&
                lines[token.map[0]] === selection
            );
            const next = view.state.doc;
            if (matches.length !== 1 || next.childCount !== 1 || !next.firstChild.isTextblock) {
                throw new Error("Only a unique single-line paragraph is supported");
            }
            lines[matches[0].map[0]] = defaultMarkdownSerializer.serialize(next);
            return frontMatter + lines.join("\n");
        }
    };
};

window.mountScaleFixture = () => {
    const paragraph = Array.from({ length: 20 }, () => "word").join(" ");
    const body = Array.from({ length: 2500 }, () => paragraph).join("\n\n");
    const view = new EditorView(document.querySelector("main"), {
        state: EditorState.create({ doc: defaultMarkdownParser.parse(body) })
    });
    view.dom.setAttribute("aria-label", "Scale fixture");
    view.dom.setAttribute("role", "textbox");
    const samples = { frame: [], work: [] };
    view.dom.addEventListener("beforeinput", () => {
        const start = performance.now();
        // "work" ends after pending tasks and a forced layout; "frame" is refresh-quantised.
        const channel = new MessageChannel();
        channel.port1.onmessage = () => {
            void document.body.offsetHeight;
            samples.work.push(performance.now() - start);
        };
        channel.port2.postMessage(null);
        requestAnimationFrame(() => requestAnimationFrame(() => {
            samples.frame.push(performance.now() - start);
        }));
    });
    return samples;
};

window.renderReview = (original, proposed) => {
    const review = document.createElement("section");
    review.setAttribute("aria-label", "Pending proposal");
    for (const [tag, label, content] of [["del", "Removed", original], ["ins", "Added", proposed]]) {
        const paragraph = document.createElement("p");
        const change = document.createElement(tag);
        change.textContent = label + ": " + content;
        paragraph.append(change);
        review.append(paragraph);
    }
    for (const [label, action] of [["Accept proposal", "accept"], ["Restore checkpoint", "restore"]]) {
        const button = document.createElement("button");
        button.textContent = label;
        button.addEventListener("click", () => window.reviewAction = action);
        review.append(button);
    }
    document.querySelector("main").append(review);
};
