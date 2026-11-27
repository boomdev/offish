const filesMin = 145;
const detailsMin = 165;
const editorMin = 280;
const dividerWidth = 8;

export function paneWidth(total, pane, other, requested) {
    const min = pane === "files" ? filesMin : detailsMin;
    const max = Math.max(min, Math.floor(total - other - editorMin - 2 * dividerWidth));
    return {min, max, width: Math.max(min, Math.min(max, Math.round(requested)))};
}

export function fitPanes(total, files, details) {
    files = paneWidth(total, "files", detailsMin, files).width;
    details = paneWidth(total, "details", files, details).width;
    return {files, details};
}
