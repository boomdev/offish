// Diagnostics for the Phase 0 embedded WebKitGTK checks. Enabled only when
// OFFISH_WEBKIT_PROBE is set on the host process.
window.runWebkitProbe = (mode, original, editor) => {
    const tauriWindow = window.__TAURI__.window.getCurrentWindow();
    const report = (label, detail) => window.__TAURI__.core.invoke("probe_report", {
        label, detail: String(detail),
    });
    const title = value => tauriWindow.setTitle(`Offish ${mode} ${value}`);
    const fail = error => {
        report(mode, `FAIL ${error?.stack || error}`);
        title("FAIL");
    };

    try {
        if (mode === "structure") {
            const names = editor.nodeNames();
            const listItems = names.filter(name => name === "ListItem").length;
            if (!names.includes("Blockquote") || listItems < 4) {
                throw new Error(`expected blockquote and nested list items: ${names.join(",")}`);
            }
            if (editor.source() !== original) throw new Error("no-op load changed source");
            if (!editor.lineClass("Quoted")?.includes("cm-blockquote") ||
                editor.lineText("Quoted").includes(">")) {
                throw new Error("blockquote did not render with source marker hidden");
            }
            editor.select("Third level");
            const selection = window.editor.state.selection.main;
            window.editor.dispatch({ changes: {
                from: selection.from, to: selection.to, insert: "Third level revised",
            } });
            const expected = original.replace("Third level", "Third level revised");
            if (editor.source() !== expected) throw new Error("bounded edit changed unrelated source");
            const operations = editor.operations();
            if (operations.length !== 1 || operations[0].replacement !== "Third level revised") {
                throw new Error(`unexpected byte operations: ${JSON.stringify(operations)}`);
            }
            const originalBytes = new TextEncoder().encode(original);
            const spliced = new Uint8Array(
                operations[0].start +
                new TextEncoder().encode(operations[0].replacement).length +
                (originalBytes.length - operations[0].end)
            );
            spliced.set(originalBytes.slice(0, operations[0].start), 0);
            const replacementBytes = new TextEncoder().encode(operations[0].replacement);
            spliced.set(replacementBytes, operations[0].start);
            spliced.set(originalBytes.slice(operations[0].end), operations[0].start + replacementBytes.length);
            if (new TextDecoder().decode(spliced) !== expected) {
                throw new Error(`byte operation did not replay cleanly: ${JSON.stringify(operations[0])}`);
            }
            report(mode, `PASS blockquote, ${listItems} list items, no-op bytes, bounded edit`);
            title("PASS");
            return;
        }

        if (mode === "review") {
            const removed = "Keep the first release on the current schedule.";
            const replacement = "Move the pilot launch to 14 December.";
            const start = new TextEncoder().encode(original.slice(0, original.indexOf(removed))).length;
            const end = start + new TextEncoder().encode(removed).length;
            const propose = action => {
                editor.showProposal(start, end, replacement);
                const button = [...document.querySelectorAll("aside button")]
                    .find(item => item.textContent === `${action} proposal`);
                if (!button) throw new Error(`${action} control missing`);
                button.focus();
                if (document.activeElement !== button) throw new Error(`${action} control cannot focus`);
                title(`${action.toLowerCase()} ready`);
            };
            propose("Reject");
            const timer = setInterval(() => {
                try {
                    if (window.reviewAction === "reject") {
                        if (editor.source() !== original) throw new Error("rejection changed source");
                        report(mode, "PASS keyboard reject preserved source");
                        window.reviewAction = undefined;
                        propose("Accept");
                    } else if (window.reviewAction === "accept") {
                        clearInterval(timer);
                        if (editor.source() !== original.replace(removed, replacement)) {
                            throw new Error("acceptance changed unrelated source");
                        }
                        report(mode, "PASS keyboard accept changed only selected source");
                        title("PASS");
                    }
                } catch (error) {
                    clearInterval(timer);
                    fail(error);
                }
            }, 50);
            return;
        }

        if (mode === "latency") {
            window.editor.destroy();
            document.querySelector("main").replaceChildren();
            document.querySelector("aside").replaceChildren();
            const samples = window.mountScaleFixture();
            const content = document.querySelector(".cm-content");
            let keys = 0, inputs = 0;
            content.addEventListener("keydown", () => {
                if (++keys === 1) report(mode, "first keydown received");
            });
            content.addEventListener("input", () => {
                if (++inputs === 1) report(mode, "first input received");
            });
            content.focus();
            report(mode, `ready focus=${document.activeElement === content}`);
            title("ready");
            let previousCount = 0;
            let lastGrowth = performance.now();
            const timer = setInterval(() => {
                if (samples.work.length !== previousCount) {
                    previousCount = samples.work.length;
                    lastGrowth = performance.now();
                }
                if (samples.work.length < 40 || samples.frame.length < samples.work.length ||
                    performance.now() - lastGrowth < 500) return;
                clearInterval(timer);
                const percentile = (values, p) => {
                    const ordered = [...values].sort((a, b) => a - b);
                    return ordered[Math.ceil(p * ordered.length) - 1].toFixed(1);
                };
                report(mode, `PASS n=${samples.work.length} work p50=${percentile(samples.work, .5)} ms p95=${percentile(samples.work, .95)} ms; second-frame p50=${percentile(samples.frame, .5)} ms p95=${percentile(samples.frame, .95)} ms`);
                title("PASS");
            }, 100);
            setTimeout(() => {
                if (samples.work.length < 40 || samples.frame.length < 40) {
                    clearInterval(timer);
                    fail(`only ${samples.work.length} work and ${samples.frame.length} frame samples`);
                }
            }, 90000);
            return;
        }

        if (mode === "ime") {
            let starts = 0, ends = 0;
            const content = document.querySelector(".cm-content");
            content.addEventListener("compositionstart", () => starts++);
            content.addEventListener("compositionend", () => {
                ends++;
                setTimeout(() => {
                    try {
                        const revised = editor.source();
                        if (!starts || !ends || revised === original) throw new Error("composition or committed edit missing");
                        window.editor.destroy();
                        document.querySelector("main").replaceChildren();
                        const reloaded = window.mountDocument(revised);
                        if (reloaded.source() !== revised) throw new Error("reload changed committed text");
                        report(mode, `PASS ${starts} start and ${ends} end events; committed edit survives reload`);
                        title("PASS");
                    } catch (error) { fail(error); }
                }, 100);
            });
            editor.select("Keep the first release on the current schedule.", 0, true);
            title("ready");
            return;
        }

        throw new Error(`unknown probe: ${mode}`);
    } catch (error) { fail(error); }
};
