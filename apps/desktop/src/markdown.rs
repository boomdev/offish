//! The coordinate and exact-byte edit contract for Offish Markdown profile v1.

use serde::{Deserialize, Serialize};
use std::{io, ops::Range};

pub const PROFILE_VERSION: u32 = 1;

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourceEdit {
    pub from: usize,
    pub to: usize,
    pub expected: String,
    pub replacement: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceEdits {
    pub profile: u32,
    pub edits: Vec<SourceEdit>,
}

pub struct SourceMap<'a> {
    source: &'a str,
    bom: usize,
    line_ending: LineEnding,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LineEnding {
    None,
    Lf,
    Crlf,
}

impl<'a> SourceMap<'a> {
    pub fn new(source: &'a str) -> io::Result<Self> {
        Ok(Self {
            source,
            bom: bom_length(source),
            line_ending: line_ending(source)?,
        })
    }

    /// Convert CodeMirror's UTF-16 positions to absolute UTF-8 file offsets.
    pub fn byte_range(&self, editor: Range<usize>) -> io::Result<Range<usize>> {
        self.map_range(editor, |(units, bytes)| (units, bytes))
    }

    /// Convert absolute UTF-8 file offsets to CodeMirror's UTF-16 positions.
    pub fn editor_range(&self, bytes: Range<usize>) -> io::Result<Range<usize>> {
        self.map_range(bytes, |(units, bytes)| (bytes, units))
    }

    fn map_range(
        &self,
        range: Range<usize>,
        orient: impl Fn((usize, usize)) -> (usize, usize),
    ) -> io::Result<Range<usize>> {
        if range.start > range.end {
            return Err(invalid("Source range is reversed."));
        }
        let body = &self.source[self.bom..];
        let mut units = 0;
        let mut start = None;
        let chars = body.char_indices().map(|(index, ch)| (index, Some(ch)));
        for (index, ch) in chars.chain([(body.len(), None)]) {
            if ch == Some('\n') && index > 0 && body.as_bytes()[index - 1] == b'\r' {
                continue;
            }
            let (position, offset) = orient((units, self.bom + index));
            if position == range.start {
                start = Some(offset);
            }
            if position == range.end {
                if let Some(start) = start {
                    return Ok(start..offset);
                }
            }
            if position > range.end {
                break;
            }
            units += ch.map_or(0, char::len_utf16);
        }
        Err(invalid(
            "Range falls outside the document or splits a Unicode character or CRLF.",
        ))
    }

    fn is_boundary(&self, offset: usize) -> bool {
        offset >= self.bom
            && self.source.is_char_boundary(offset)
            && !(offset > 0
                && self.source.as_bytes().get(offset) == Some(&b'\n')
                && self.source.as_bytes()[offset - 1] == b'\r')
    }
}

impl SourceEdits {
    /// Apply sorted, disjoint splices to the same captured source.
    /// Structural validation for visual edits belongs to the Markdown parser.
    pub fn apply(&self, source: &str, limit: usize) -> io::Result<String> {
        if self.profile != PROFILE_VERSION {
            return Err(invalid("Unsupported Markdown source profile."));
        }
        if source.len() > limit || source.contains('\0') {
            return Err(invalid("Source exceeds the size limit or contains NUL."));
        }
        if self.edits.is_empty() {
            return Ok(source.into());
        }

        let map = SourceMap::new(source)?;
        let mut last_start = None;
        let mut previous_end = 0;
        let mut size = source.len();
        for edit in &self.edits {
            if edit.from < previous_end || last_start == Some(edit.from) {
                return Err(invalid(
                    "Source edits overlap, repeat an anchor, or are out of order.",
                ));
            }
            if !map.is_boundary(edit.from) || !map.is_boundary(edit.to) {
                return Err(invalid(
                    "Source edit splits a character, a CRLF, or the BOM.",
                ));
            }
            let Some(expected) = source.get(edit.from..edit.to) else {
                return Err(invalid("Invalid source edit range."));
            };
            if expected != edit.expected {
                return Err(invalid("Source edit does not match the captured text."));
            }
            let replacement_ending = line_ending(&edit.replacement)?;
            let expected_ending = if map.line_ending == LineEnding::Crlf {
                LineEnding::Crlf
            } else {
                LineEnding::Lf
            };
            if replacement_ending != LineEnding::None && replacement_ending != expected_ending {
                return Err(invalid("Replacement must use the source line endings."));
            }
            size = size
                .checked_sub(expected.len())
                .and_then(|size| size.checked_add(edit.replacement.len()))
                .ok_or_else(|| invalid("Source edit size overflow."))?;
            last_start = Some(edit.from);
            previous_end = edit.to;
        }
        if size > limit {
            return Err(invalid("Edited Markdown exceeds the size limit."));
        }

        let mut output = String::with_capacity(size);
        let mut previous_end = 0;
        for edit in &self.edits {
            output.push_str(&source[previous_end..edit.from]);
            output.push_str(&edit.replacement);
            previous_end = edit.to;
        }
        output.push_str(&source[previous_end..]);
        if bom_length(&output) != map.bom {
            return Err(invalid("Source edit changes the leading BOM."));
        }
        Ok(output)
    }
}

/// Full span of a leading, fenced front matter block in profile v1.
pub fn front_matter_range(source: &str) -> Option<Range<usize>> {
    let start = bom_length(source);
    let mut lines = source[start..].split_inclusive('\n');
    let first = lines.next()?;
    if delimiter(first) != "---" {
        return None;
    }
    let mut end = start + first.len();
    for line in lines {
        end += line.len();
        if matches!(delimiter(line), "---" | "...") {
            return Some(start..end);
        }
    }
    None
}

fn delimiter(line: &str) -> &str {
    line.strip_suffix('\n')
        .map_or(line, |body| body.strip_suffix('\r').unwrap_or(body))
}

fn bom_length(source: &str) -> usize {
    if source.starts_with('\u{feff}') {
        '\u{feff}'.len_utf8()
    } else {
        0
    }
}

fn line_ending(source: &str) -> io::Result<LineEnding> {
    let bytes = source.as_bytes();
    let mut crlf = false;
    let mut lf = false;
    for (index, byte) in bytes.iter().copied().enumerate() {
        match byte {
            0 => return Err(invalid("Markdown source contains NUL.")),
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => crlf = true,
            b'\r' => {
                return Err(invalid(
                    "Standalone CR line endings require read-only mode.",
                ));
            }
            b'\n' if index == 0 || bytes[index - 1] != b'\r' => lf = true,
            _ => (),
        }
    }
    match (crlf, lf) {
        (true, true) => Err(invalid("Mixed line endings require read-only mode.")),
        (true, false) => Ok(LineEnding::Crlf),
        (false, true) => Ok(LineEnding::Lf),
        (false, false) => Ok(LineEnding::None),
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
