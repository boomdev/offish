window.__TAURI__.core.invoke("probe").then(title => {
    window.exercise("# Offish pilot\n\nKeep the first release on the current schedule.\n");
    document.title = title;
    window.__TAURI__.window.getCurrentWindow().setTitle(title);
    document.querySelector("main").addEventListener("input", () => {
        if (document.querySelector("main").textContent.includes(".x")) {
            window.__TAURI__.window.getCurrentWindow().setTitle("Offish WebView edit confirmed");
        }
    });
});
