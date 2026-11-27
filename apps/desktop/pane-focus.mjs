const workspaceOrder = ["files", "files-divider", "editor", "details-divider", "details"];

export function paneDestination(current, last, key) {
    if (key === "ArrowUp") return "menu";
    if (key === "ArrowDown") return current === "menu" ? last : null;
    if (key !== "ArrowLeft" && key !== "ArrowRight") return null;
    const index = workspaceOrder.indexOf(current);
    if (index < 0) return null;
    return workspaceOrder[index + (key === "ArrowLeft" ? -1 : 1)] ?? null;
}

export function ancestorFolders(path) {
    const parts = path.split("/");
    return parts.slice(0, -1).map((_, index) => parts.slice(0, index + 1).join("/"));
}
