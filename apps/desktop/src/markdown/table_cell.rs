//! Authoritative spans for the conservative top-level table subset in Simple mode.

use super::{PROFILE_VERSION, SourceEdit, SourceEdits, SourceMap, front_matter_range, invalid};
use serde::{Deserialize, Serialize};
use std::{io, ops::Range};

const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
const CELL_BREAK: &str = "<br>";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TableCellAddress {
    pub table_from: usize,
    pub table_to: usize,
    pub row: usize,
    pub column: usize,
}

#[derive(Serialize)]
pub struct TableCell {
    pub from: usize,
    pub to: usize,
    pub text: String,
}

/// Return editable text with source break tags decoded to line feeds.
pub fn inspect(source: &str, address: &TableCellAddress) -> io::Result<TableCell> {
    let (_, cell) = locate(source, address)?;
    let editor = SourceMap::new(source)?.editor_range(cell.clone())?;
    Ok(TableCell {
        from: editor.start,
        to: editor.end,
        text: source[cell].replace(CELL_BREAK, "\n"),
    })
}

/// Return the validated Markdown splice with line feeds encoded as break tags.
pub fn edit(source: &str, address: &TableCellAddress, replacement: &str) -> io::Result<TableCell> {
    if !replacement.split('\n').all(plain_text) {
        return Err(invalid(
            "Enter plain text. Use Shift+Enter for line breaks.",
        ));
    }
    let replacement = replacement.replace('\n', CELL_BREAK);
    if replacement != replacement.trim() {
        return Err(invalid("Remove whitespace at the edges of the cell."));
    }
    let (table, cell) = locate(source, address)?;
    let offset = SourceMap::new(source)?.editor_range(cell.clone())?;
    let old_len = cell.len();
    let updated = SourceEdits {
        profile: PROFILE_VERSION,
        edits: vec![SourceEdit {
            from: cell.start,
            to: cell.end,
            expected: source[cell].into(),
            replacement: replacement.clone(),
        }],
    }
    .apply(source, MAX_SOURCE_BYTES)?;
    let updated_table = table.start..table.end - old_len + replacement.len();
    let updated_cell = cell_range(&updated, updated_table, address.row, address.column)?;
    if updated[updated_cell] != replacement {
        return Err(invalid("The replacement changes the table structure."));
    }
    Ok(TableCell {
        from: offset.start,
        to: offset.end,
        text: replacement,
    })
}

fn locate(source: &str, address: &TableCellAddress) -> io::Result<(Range<usize>, Range<usize>)> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(invalid("Markdown exceeds the size limit."));
    }
    let table = SourceMap::new(source)?.byte_range(address.table_from..address.table_to)?;
    let cell = cell_range(source, table.clone(), address.row, address.column)?;
    let text = &source[cell.clone()];
    if text != text.trim() || !text.split(CELL_BREAK).all(plain_text) {
        return Err(invalid(
            "This cell contains Markdown syntax. Edit it in Source mode.",
        ));
    }
    Ok((table, cell))
}

fn plain_text(text: &str) -> bool {
    !text.chars().any(|ch| {
        ch.is_control()
            || matches!(
                ch,
                '|' | '\\' | '<' | '>' | '`' | '*' | '_' | '[' | ']' | '~' | '&'
            )
    })
}

fn may_open_html(line: &str) -> bool {
    // A break inside a pipe-delimited row cannot open an HTML block.
    line.contains('<')
        && (!line.starts_with('|')
            || !line.ends_with('|')
            || line.split(CELL_BREAK).any(|part| part.contains('<')))
}

fn cell_range(
    source: &str,
    table: Range<usize>,
    row: usize,
    column: usize,
) -> io::Result<Range<usize>> {
    let unsupported =
        || invalid("This table is outside the Simple mode subset. Edit it in Source mode.");
    let start = front_matter_range(source).map_or_else(
        || if source.starts_with('\u{feff}') { 3 } else { 0 },
        |metadata| metadata.end,
    );
    if table.start < start {
        return Err(unsupported());
    }
    let prefix = &source[start..table.start];
    let suffix = &source[table.end..];
    // A blank line or the beginning of the Markdown body separates this table
    // from surrounding blocks. Abstain after possible opaque HTML or fences.
    if (!prefix.is_empty() && !prefix.ends_with('\n'))
        || prefix
            .lines()
            .last()
            .is_some_and(|line| !line.trim().is_empty())
        || prefix.contains('\0')
        || prefix.lines().any(may_open_html)
        || prefix.contains("```")
        || prefix.contains("~~~")
        || (!suffix.is_empty() && !suffix.starts_with(['\r', '\n']))
        || suffix
            .lines()
            .nth(1)
            .is_some_and(|line| !line.trim().is_empty())
    {
        return Err(unsupported());
    }

    let block = &source[table.clone()];
    let mut offset = table.start;
    let mut columns = None;
    let mut selected = None;
    let target_line = if row == 0 {
        0
    } else {
        row.checked_add(1).ok_or_else(unsupported)?
    };
    let mut line_count = 0;
    for (index, raw) in block.split_inclusive('\n').enumerate() {
        line_count += 1;
        let line = raw.trim_end_matches(['\r', '\n']);
        let inside = line
            .strip_prefix('|')
            .and_then(|line| line.strip_suffix('|'))
            .ok_or_else(unsupported)?;
        // Escaped pipes are outside this first subset.
        if inside.contains('\\') {
            return Err(unsupported());
        }
        let mut count = 0;
        let mut position = offset + 1;
        for (current, cell) in inside.split('|').enumerate() {
            let value = cell.trim_matches([' ', '\t']);
            if index == 1 {
                let dashes = value.strip_prefix(':').unwrap_or(value);
                let dashes = dashes.strip_suffix(':').unwrap_or(dashes);
                if dashes.len() < 3 || !dashes.bytes().all(|byte| byte == b'-') {
                    return Err(unsupported());
                }
            } else if index == target_line && current == column {
                let from = position + cell.len() - cell.trim_start_matches([' ', '\t']).len();
                selected = Some(from..from + value.len());
            }
            position += cell.len() + 1;
            count += 1;
        }
        if columns.is_some_and(|expected| count != expected) {
            return Err(unsupported());
        }
        columns = Some(count);
        offset += raw.len();
    }
    if line_count < 2 {
        return Err(unsupported());
    }
    selected.ok_or_else(unsupported)
}
