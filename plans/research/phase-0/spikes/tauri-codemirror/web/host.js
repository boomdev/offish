window.__TAURI__.core.invoke("boot").then(boot => {
    window.offishAssetBase = boot.asset_base;
    const editor = window.mountDocument(boot.source);
    document.title = boot.title;
    window.__TAURI__.window.getCurrentWindow().setTitle(boot.title);

    // Confirm keyboard input reaches the live preview inside WebKitGTK.
    // Probe modes own their title so late input cannot hide a probe result.
    if (!boot.probe) {
        document.querySelector("main").addEventListener("input", () => {
            if (editor.source().includes(".x") || editor.source().includes("x")) {
                const title = "Offish CodeMirror edit confirmed";
                document.title = title;
                window.__TAURI__.window.getCurrentWindow().setTitle(title);
            }
        });
    }

    // Expose a handle for manual checks of the workspace image.
    window.spike = {
        imageLoaded: () => {
            const image = document.querySelector('img[alt="Pilot timeline"]');
            return Boolean(image && image.complete && image.naturalWidth > 0);
        },
        source: () => editor.source(),
    };
    if (boot.probe) window.runWebkitProbe(boot.probe, boot.source, editor);
}).catch(error => {
    document.title = "Offish CodeMirror boot failed";
    console.error(error);
});
