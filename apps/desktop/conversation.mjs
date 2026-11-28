// Local conversation UI. Approval grants refer to host-owned snapshots.

const $ = id => document.getElementById(id);
const MAX_MENTIONS = 6;
const HISTORY_LIMIT = 20;
const POLL_DELAY = 400;
const providerName = value => value === "codex" ? "Codex" : "Offline test provider";

export function setupConversation({invoke, getSelection, labelForPath, reviewProposal}) {
    const prompt = $("conversation-prompt");
    const provider = $("conversation-provider");
    const mode = $("conversation-mode");
    const model = $("conversation-model");
    const effort = $("conversation-effort");
    const fast = $("conversation-fast");
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
    let accountBusy = false;
    let connected = false;
    let models = [];
    const status = message => {
        $("conversation-status").textContent = message;
    };

    function render() {
        const locked = busy || Boolean(pending || running);
        const codex = provider.value === "codex";
        prompt.readOnly = locked;
        provider.disabled = locked || accountBusy;
        mode.disabled = locked;
        model.disabled = effort.disabled = fast.disabled = locked || accountBusy;
        $("conversation-send").disabled = locked || (codex && (!connected || accountBusy));
        $("conversation-model-options").hidden = !codex || !connected;
        $("conversation-account").hidden = !codex;
        $("conversation-connect").disabled = locked || accountBusy;
        $("conversation-connect").textContent = connected ? "Reconnect ChatGPT" : "Continue with ChatGPT";
        $("conversation-disconnect").hidden = !connected;
        $("conversation-disconnect").disabled = locked || accountBusy;
        $("conversation-prompt-label").textContent = codex ? "Message to Codex" : "Message to offline test provider";
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
            reply.textContent = message.answer ? `${providerName(message.provider)}: ${message.answer}` : "";
            reply.hidden = !message.answer;
            const summary = document.createElement("small");
            const outcome = running?.id === message.id ? "Streaming" : {
                interrupted: "Interrupted",
                cancelled: "Cancelled",
                complete: "Complete",
                stale: "Proposal unavailable",
                failed: "Failed",
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

    function option(value, label) {
        const entry = document.createElement("option");
        entry.value = value;
        entry.textContent = label;
        return entry;
    }

    function updateEffort() {
        const choice = models.find(entry => entry.model === model.value);
        const defaults = choice?.default_effort;
        effort.replaceChildren(option("", defaults ? `Default (${defaults})` : "Default"),
            ...(choice?.efforts ?? []).map(value => option(value, value)));
        effort.value = "";
        fast.checked = false;
        $("conversation-effort-label").hidden = !choice?.efforts.length;
        $("conversation-fast-label").hidden = !choice?.fast;
    }

    async function loadModels() {
        models = [];
        model.replaceChildren(option("", "Pilot default"));
        model.value = "";
        updateEffort();
        if (!connected) return;
        try {
            const catalog = await invoke("codex_models");
            models = catalog.models;
            model.replaceChildren(option("", "Pilot default"),
                ...models.map(entry => option(entry.model, entry.name)));
            model.value = models.find(entry => entry.model === "gpt-6.1-sol")?.model ?? models[0]?.model ?? "";
            updateEffort();
            if (!catalog.account_specific) {
                status("Using the Codex model catalog. ChatGPT checks access when you send.");
            }
        } catch (error) {
            status(String(error));
        }
    }

    model.onchange = updateEffort;
    mode.onchange = () => {
        prompt.placeholder = mode.value === "edit" ? "Describe the changes to make." : "Ask about the document.";
    };

    provider.onchange = async () => {
        status("");
        render();
        if (provider.value !== "codex") return;
        accountBusy = true;
        render();
        try {
            connected = await invoke("chatgpt_connected");
            await loadModels();
        } catch (error) {
            connected = false;
            status(String(error));
        } finally {
            accountBusy = false;
            render();
        }
    };

    async function manageAccount(connect) {
        if (busy || pending || running || accountBusy) return;
        accountBusy = true;
        if (connect) status("Finish signing in to ChatGPT in your browser.");
        render();
        try {
            await invoke(connect ? "connect_chatgpt" : "disconnect_chatgpt");
            connected = connect;
            status(connect ? "ChatGPT connected." : "ChatGPT disconnected.");
            await loadModels();
        } catch (error) {
            status(String(error));
        } finally {
            accountBusy = false;
            render();
        }
    }

    $("conversation-connect").onclick = () => manageAccount(true);
    $("conversation-disconnect").onclick = () => manageAccount(false);

    $("conversation-form").onsubmit = async event => {
        event.preventDefault();
        if (busy || pending || running) return;
        if (provider.value === "codex" && (!connected || accountBusy)) return;
        const selection = getSelection(mode.value);
        if (selection === undefined) {
            status("Make sure the active Markdown document is saved and ready before sending.");
            return;
        }
        if (mode.value === "edit" && !selection) {
            status("Open a saved Markdown document to request an edit.");
            return;
        }
        const current = generation;
        busy = true;
        status("Preparing context…");
        render();
        try {
            const prepared = await invoke("prepare_agent_turn", {request: {
                prompt: prompt.value, selection, mentions, provider: provider.value, mode: mode.value,
                settings: provider.value === "codex" ? {
                    model: model.value || null, effort: effort.value || null, fast: fast.checked,
                } : {},
            }});
            if (current !== generation) return;
            pending = prepared.id;
            $("context-intro").textContent = prepared.provider === "codex" ?
                "This turn sends your message and the exact text shown here to OpenAI using your ChatGPT plan." :
                "The offline test provider receives the message and exact text shown here.";
            const task = [prepared.mode === "edit" ? "Edit" : "Ask"];
            if (prepared.provider === "codex") {
                const settings = prepared.settings;
                task.push(models.find(entry => entry.model === settings?.model)?.name ??
                    settings?.model ?? "Pilot default");
                if (settings?.effort) task.push(`Effort ${settings.effort}`);
                if (settings?.fast) task.push("Fast mode");
            }
            $("context-task").textContent = task.join(" · ");
            const items = $("context-items");
            items.replaceChildren();
            appendExcerpt(items, "Message", prepared.prompt);
            if (prepared.selection) {
                const label = prepared.selection.whole ?
                    (prepared.mode === "edit" ? "Document body" : "Whole document") : "Selection";
                appendExcerpt(items, `${label} in ${labelForPath(prepared.selection.path)}`, prepared.selection.text);
            }
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
                if (update.type === "waiting") {
                    schedulePoll(message);
                    return;
                }
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
                    if (staged) {
                        renderHistory();
                        await showStaged();
                        return;
                    }
                    status(update.message.outcome === "complete" ? "Answer ready." :
                        "The edit could not be staged. Check the saved document and try again.");
                } else if (update.type === "failed") {
                    history[history.indexOf(message)] = update.message;
                    status(update.error);
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
            status(`Waiting for ${providerName(message.provider)}…`);
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

    async function showStaged() {
        const proposal = staged;
        const current = generation;
        if (!proposal || busy) return;
        busy = true;
        render();
        try {
            await reviewProposal(proposal);
            if (current === generation) {
                staged = null;
                status("Proposal opened in the editor.");
            }
        } catch (error) {
            if (current === generation) status(`Proposal ready. ${error}`);
        } finally {
            if (current === generation) {
                busy = false;
                render();
            }
        }
    }
    $("conversation-review").onclick = showStaged;

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
        provider.value = "offline";
        mode.value = "ask";
        mode.onchange();
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
