export function pathInFolder(folder, name) {
    return folder ? folder + "/" + name : name;
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
