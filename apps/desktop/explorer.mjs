export function pathInFolder(folder, name) {
    return folder ? folder + "/" + name : name;
}

export function displayPath(path, roots) {
    const index = path.indexOf("/");
    const key = index === -1 ? path : path.slice(0, index);
    const root = roots.find(entry => entry.name === key);
    return root?.label ? root.label + (index === -1 ? "" : path.slice(index)) : path;
}

export function workspaceFilePath(path, roots) {
    const separator = path.indexOf("/");
    const key = separator < 0 ? path : path.slice(0, separator);
    const root = roots.find(entry => entry.name === key)?.hint;
    if (!root) return null;
    return separator < 0 ? root : root.replace(/\/$/, "") + path.slice(separator);
}

export function renamedPath(path, from, to) {
    if (path === from) return to;
    return path.startsWith(from + "/") ? to + path.slice(from.length) : path;
}

export function remapOpenDocuments(documents, from, to) {
    const entries = [...documents].map(([path, item]) => [renamedPath(path, from, to), item]);
    if (new Set(entries.map(([path]) => path)).size !== entries.length) {
        throw new Error("Close the tab at the destination before renaming.");
    }
    return entries;
}

export function isMarkdown(path) {
    const name = path.split("/").at(-1);
    return /.+\.(?:md|markdown)$/i.test(name);
}

export function searchSelection(doc, {line: number, column, matched}) {
    const line = doc.line(Math.min(number, doc.lines));
    const anchor = line.from + column;
    const head = anchor + matched.length;
    if (line.number !== number || head > line.to || doc.sliceString(anchor, head) !== matched) {
        return {anchor: line.from};
    }
    return {anchor, head};
}

export function treeCommand(rows, index, key, shift = false, actionFocused = false) {
    const row = rows[index];
    if (!row || row.disabled) return null;
    if (key === "ArrowUp" || key === "ArrowDown") {
        const step = key === "ArrowUp" ? -1 : 1;
        for (let next = index + step; next >= 0 && next < rows.length; next += step) {
            if (!rows[next].disabled) return {kind: "focus", path: rows[next].path};
        }
        return {kind: "stay"};
    }
    if (key === "ArrowLeft") {
        if (row.expanded) return {kind: "activate"};
        const parent = row.path.slice(0, Math.max(0, row.path.lastIndexOf("/")));
        return parent ? {kind: "focus", path: parent} : {kind: "stay"};
    }
    if (key === "ArrowRight") {
        if (row.expanded === false) return {kind: "activate"};
        const child = rows.find((next, at) => at > index && !next.disabled &&
            next.path.startsWith(row.path + "/") && !next.path.slice(row.path.length + 1).includes("/"));
        return row.expanded && child ? {kind: "focus", path: child.path} : {kind: "stay"};
    }
    if (key === "Enter") return actionFocused ? null : {kind: "activate"};
    if (row.root) return null;
    if (key === "Delete") return {kind: shift ? "delete" : "trash"};
    if (key === "F2") return {kind: "rename"};
    if (key === "ContextMenu" || key === "F10" && shift) return {kind: "menu"};
    return null;
}
