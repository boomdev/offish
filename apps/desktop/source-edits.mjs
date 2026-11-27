const encoder = new TextEncoder();

function splitsUnit(text, index) {
    if (index <= 0 || index >= text.length) return false;
    const before = text.charCodeAt(index - 1);
    const after = text.charCodeAt(index);
    return (before === 13 && after === 10) ||
        (before >= 0xd800 && before <= 0xdbff && after >= 0xdc00 && after <= 0xdfff);
}

// Compute one splice on the captured revision. Equal bytes between separate
// changes replay verbatim inside the splice.
export function sourceEditBatch(source, updated) {
    if (source === updated) return {profile: 1, edits: []};
    let start = 0;
    while (start < source.length && start < updated.length && source[start] === updated[start]) start++;
    while (splitsUnit(source, start) || splitsUnit(updated, start)) start--;

    let end = source.length;
    let updatedEnd = updated.length;
    while (end > start && updatedEnd > start && source[end - 1] === updated[updatedEnd - 1]) {
        end--;
        updatedEnd--;
    }
    while (splitsUnit(source, end) || splitsUnit(updated, updatedEnd)) {
        end++;
        updatedEnd++;
    }
    const expected = source.slice(start, end);
    const from = encoder.encode(source.slice(0, start)).length;
    const to = from + encoder.encode(expected).length;
    return {profile: 1, edits: [{from, to, expected, replacement: updated.slice(start, updatedEnd)}]};
}
