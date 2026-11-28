// Local conversation UI. Approval grants refer to host-owned snapshots.

const $ = id => document.getElementById(id);
const MAX_MENTIONS = 6;
const HISTORY_LIMIT = 20;
const POLL_DELAY = 400;

export function setupConversation({invoke, getSelection, labelForPath, reviewProposal}) {
    const prompt = $("conversation-prompt");
    const context = $("context-dialog");
    const picker = $("mention-dialog");
    const confirmDelete = $("conversation-delete");
    let generation = 0;
    let history = [];
    let mentions = [];
    let pending = null;
    let running = null;
    let staged = null;
    let busy = false;
    let directory = "";
    let listing = 0;
    let deleteGeneration = -1;
    const status = message => {
        $("conversation-status").textContent = message;
    };

    function render() {
        const locked = busy || Boolean(pending || running);
        prompt.readOnly = locked;
        $("conversation-send").disabled = locked;
        $("conversation-mention").disabled = locked || mentions.length === MAX_MENTIONS;
        $("conversation-clear").hidden = history.length === 0;
        $("conversation-clear").disabled = locked;
        $("conversation-cancel").hidden = !running;
        $("conversation-review").hidden = !staged;
        $("conversation-review").disabled = locked;
        $("context-approve").disabled = busy;
        $("context-deny").disabled = busy;
        for (const remove of $("conversation-mentions").querySelectorAll("button")) {
            remove.disabled = locked;
        }
    }

    function renderMentions() {
        const list = $("conversation-mentions");
        list.hidden = mentions.length === 0;
        list.replaceChildren(...mentions.map(mention => {
            const name = labelForPath(mention.path);
            const item = document.createElement("li");
            const label = document.createElement("span");
            label.textContent = `@${name}${mention.directory ? "/" : ""}`;
            const remove = document.createElement("button");
            remove.type = "button";
            remove.textContent = "Remove";
            remove.setAttribute("aria-label", `Remove @${name}`);
            remove.onclick = () => {
                if (busy || pending || running) return;
                mentions = mentions.filter(existing => existing !== mention);
                renderMentions();
                render();
                $("conversation-mention").focus();
            };
            item.append(label, remove);
            return item;
        }));
    }

    function renderHistory() {
        const list = $("conversation-history");
        list.hidden = history.length === 0;
        list.replaceChildren(...history.map(message => {
            const entry = document.createElement("li");
            const text = document.createElement("p");
            text.textContent = `You: ${message.prompt}`;
            const reply = document.createElement("p");
            reply.textContent = message.answer ? `Offline test provider: ${message.answer}` : "";
            reply.hidden = !message.answer;
            const summary = document.createElement("small");
            const outcome = running?.id === message.id ? "Streaming" : {
                interrupted: "Interrupted",
                cancelled: "Cancelled",
                complete: "Complete",
                stale: "Proposal unavailable",
            }[message.outcome];
            summary.textContent = `${outcome} · ${message.references} approved ${message.references === 1 ? "reference" : "references"}`;
            entry.append(text, reply, summary);
            return entry;
        }));
        list.scrollTop = list.scrollHeight;
    }

    function mention(path, isDirectory) {
        if (mentions.length >= MAX_MENTIONS) return;
        if (!mentions.some(entry => entry.path === path)) {
            mentions.push({path, directory: isDirectory});
        }
        picker.close();
        renderMentions();
        render();
        prompt.focus();
    }

    async function listFolder(path) {
        const request = ++listing;
        const entries = $("mention-entries");
        entries.replaceChildren();
        $("mention-path").textContent = "Loading…";
        $("mention-folder").disabled = true;
        try {
            const children = await invoke("list_directory", {path});
            if (!picker.open || request !== listing) return;
            directory = path;
            const rows = children.filter(entry => !entry.name.startsWith(".") &&
                ["root", "folder", "reference", "markdown"].includes(entry.kind));
            $("mention-path").textContent = path ? labelForPath(path) : "Workspace roots";
            $("mention-up").hidden = $("mention-folder").hidden = !path;
            $("mention-folder").disabled = !path;
            entries.replaceChildren(...rows.map(entry => {
                const child = path ? `${path}/${entry.name}` : entry.name;
                const isFolder = entry.kind === "root" || entry.kind === "folder";
                const item = document.createElement("li");
                const button = document.createElement("button");
                button.type = "button";
                button.textContent = `${entry.label ?? entry.name}${isFolder ? "/" : ""}`;
                button.setAttribute("aria-label", isFolder ? `Browse ${entry.label ?? entry.name}` : `Mention ${entry.name}`);
                button.onclick = () => isFolder ? listFolder(child) : mention(child, false);
                item.append(button);
                return item;
            }));
            (entries.querySelector("button") ?? $("mention-folder")).focus();
        } catch (error) {
            if (picker.open && request === listing) $("mention-path").textContent = String(error);
        }
    }

    $("conversation-mention").onclick = () => {
        if (busy || pending || running) return;
        directory = "";
        $("mention-up").hidden = $("mention-folder").hidden = true;
        picker.showModal();
        listFolder("");
    };
    $("mention-up").onclick = () => listFolder(directory.substring(0, directory.lastIndexOf("/")));
    $("mention-folder").onclick = () => mention(directory, true);
    $("mention-close").onclick = () => picker.close();

    function appendExcerpt(container, title, text) {
        const label = document.createElement("h3");
        label.textContent = title;
        const excerpt = document.createElement("pre");
        excerpt.textContent = text;
        container.append(label, excerpt);
    }

    $("conversation-form").onsubmit = async event => {
        event.preventDefault();
        if (busy || pending || running) return;
        const selection = getSelection();
        if (!selection) {
            status("Select text in a saved Markdown document before preparing context.");
            return;
        }
        const current = generation;
        busy = true;
        status("Preparing context…");
        render();
        try {
            const prepared = await invoke("prepare_agent_turn", {request: {
                prompt: prompt.value, selection, mentions,
            }});
            if (current !== generation) return;
            pending = prepared.id;
            const items = $("context-items");
            items.replaceChildren();
            appendExcerpt(items, "Message", prepared.prompt);
            appendExcerpt(items, `Selection in ${labelForPath(prepared.selection.path)}`, prepared.selection.text);
            for (const file of prepared.files) appendExcerpt(items, labelForPath(file.path), file.text);
            $("context-limit").hidden = !prepared.limited;
            status("");
            context.showModal();
        } catch (error) {
            if (current === generation) status(String(error));
        } finally {
            if (current === generation) {
                busy = false;
                render();
            }
        }
    };

    context.addEventListener("cancel", event => {
        if (busy) event.preventDefault();
    });
    $("context-deny").onclick = () => context.close();
    context.addEventListener("close", async () => {
        $("context-items").replaceChildren();
        if (!pending) return;
        const id = pending;
        const current = generation;
        pending = null;
        busy = true;
        render();
        try {
            await invoke("cancel_agent_turn", {id});
            if (current === generation) status("Context was declined.");
        } catch (error) {
            if (current === generation) status(String(error));
        } finally {
            if (current === generation) {
                busy = false;
                render();
                prompt.focus();
            }
        }
    });

    function schedulePoll(message) {
        setTimeout(async () => {
            if (running !== message) return;
            try {
                const update = await invoke("poll_agent_turn", {id: message.id});
                if (running !== message) return;
                if (update.type === "delta") {
                    message.answer += update.text;
                    renderHistory();
                    schedulePoll(message);
                    return;
                }
                const wasFocused = document.activeElement === $("conversation-cancel");
                running = null;
                if (update.type === "done") {
                    history[history.indexOf(message)] = update.message;
                    staged = update.staged;
                    status(staged ? "Proposal ready for review." : "The edit could not be staged. Select the current text and try again.");
                } else {
                    status("The turn was interrupted.");
                }
                renderHistory();
                render();
                if (wasFocused) (staged ? $("conversation-review") : prompt).focus();
            } catch (error) {
                if (running !== message) return;
                running = null;
                invoke("cancel_agent_turn", {id: message.id}).catch(() => {});
                status(String(error));
                renderHistory();
                render();
                prompt.focus();
            }
        }, POLL_DELAY);
    }

    $("context-approve").onclick = async () => {
        if (!pending || busy) return;
        const current = generation;
        const id = pending;
        busy = true;
        render();
        try {
            const message = await invoke("approve_agent_turn", {id});
            if (current !== generation) return;
            running = message;
            staged = null;
            history = [...history, message].slice(-HISTORY_LIMIT);
            prompt.value = "";
            mentions = [];
            status("Streaming from the offline test provider…");
            renderMentions();
            renderHistory();
            schedulePoll(message);
        } catch (error) {
            if (current === generation) status(String(error));
        } finally {
            if (current === generation) {
                pending = null;
                busy = false;
                context.close();
                render();
                (running ? $("conversation-cancel") : prompt).focus();
            }
        }
    };

    $("conversation-cancel").onclick = async () => {
        if (!running) return;
        const message = running;
        const current = generation;
        running = null;
        busy = true;
        render();
        try {
            const result = await invoke("cancel_agent_turn", {id: message.id});
            if (current !== generation) return;
            if (result) history[history.indexOf(message)] = result;
            status(result?.outcome === "complete" ?
                "The turn finished before cancellation. Its proposal was dismissed." : "Turn cancelled.");
        } catch (error) {
            if (current === generation) status(String(error));
        } finally {
            if (current === generation) {
                busy = false;
                renderHistory();
                render();
                prompt.focus();
            }
        }
    };

    $("conversation-review").onclick = async () => {
        const proposal = staged;
        const current = generation;
        if (!proposal || busy) return;
        busy = true;
        render();
        try {
            await reviewProposal(proposal);
            if (current === generation) {
                staged = null;
                status("");
            }
        } catch (error) {
            if (current === generation) status(String(error));
        } finally {
            if (current === generation) {
                busy = false;
                render();
            }
        }
    };

    $("conversation-clear").onclick = () => {
        deleteGeneration = generation;
        confirmDelete.returnValue = "cancel";
        confirmDelete.showModal();
    };
    confirmDelete.addEventListener("close", async () => {
        if (confirmDelete.returnValue !== "clear" || deleteGeneration !== generation) return;
        const current = generation;
        busy = true;
        render();
        try {
            await invoke("clear_conversation");
            if (current !== generation) return;
            staged = null;
            history = [];
            renderHistory();
            status("Conversation cleared.");
        } catch (error) {
            if (current === generation) status(String(error));
        } finally {
            if (current === generation) {
                busy = false;
                render();
                prompt.focus();
            }
        }
    });

    return async function reset(enabled) {
        const current = ++generation;
        listing++;
        history = [];
        mentions = [];
        pending = running = staged = null;
        busy = Boolean(enabled);
        prompt.value = "";
        for (const dialog of [picker, context, confirmDelete]) {
            if (dialog.open) dialog.close();
        }
        $("context-items").replaceChildren();
        status("");
        renderMentions();
        renderHistory();
        render();
        if (!enabled) return;
        try {
            const stored = await invoke("conversation_history");
            if (current !== generation) return;
            history = stored;
            renderHistory();
        } catch (error) {
            if (current === generation) status(String(error));
        } finally {
            if (current === generation) {
                busy = false;
                render();
            }
        }
    };
}
