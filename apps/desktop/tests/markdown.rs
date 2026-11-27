use offish::markdown::{PROFILE_VERSION, SourceEdit, SourceEdits, SourceMap, front_matter_range};

const BRIEF: &str = include_str!("../../../plans/research/phase-0/fixtures/pilot/brief.md");
const NESTED: &str = include_str!("../../../plans/research/phase-0/fixtures/pilot/nested.md");

fn batch(edits: Vec<SourceEdit>) -> SourceEdits {
    SourceEdits {
        profile: PROFILE_VERSION,
        edits,
    }
}

fn edit(from: usize, to: usize, expected: &str, replacement: &str) -> SourceEdit {
    SourceEdit {
        from,
        to,
        expected: expected.into(),
        replacement: replacement.into(),
    }
}

#[test]
fn code_mirror_ranges_map_across_bom_surrogate_pairs_and_crlf() {
    let source = "\u{feff}A😀\r\n日本語\r\n";
    let map = SourceMap::new(source).unwrap();
    for (editor, bytes) in [
        (0..0, 3..3),
        (1..3, 4..8),
        (3..4, 8..10),
        (4..7, 10..19),
        (8..8, 21..21),
    ] {
        assert_eq!(map.byte_range(editor.clone()).unwrap(), bytes);
        assert_eq!(map.editor_range(bytes).unwrap(), editor);
    }
    for range in [2..2, 7..9, std::ops::Range { start: 3, end: 1 }] {
        assert!(map.byte_range(range).is_err());
    }
    for range in [0..3, 5..8, 9..10, 21..22] {
        assert!(map.editor_range(range).is_err());
    }
    assert_eq!(SourceMap::new("").unwrap().byte_range(0..0).unwrap(), 0..0);
    assert_eq!(
        SourceMap::new("\u{feff}")
            .unwrap()
            .byte_range(0..0)
            .unwrap(),
        3..3
    );
}

#[test]
fn matching_pilot_spans_replay_exactly_in_lf_crlf_and_with_bom() {
    for newline in ["\n", "\r\n"] {
        for bom in ["", "\u{feff}"] {
            for (fixture, changes) in [
                (
                    BRIEF,
                    [
                        ("Pilot launch brief", "Pilot launch note"),
                        ("日本語", "日本語😀"),
                    ],
                ),
                (NESTED, [("bold", "strong"), ("reference", "briefing")]),
            ] {
                let source = format!("{bom}{}", fixture.replace('\n', newline));
                let map = SourceMap::new(&source).unwrap();
                let mut edits = Vec::new();
                let mut expected = source.clone();
                for (before, after) in changes {
                    let offset = source.find(before).unwrap();
                    assert_eq!(source.matches(before).count(), 1);
                    let editor_start = source[bom.len()..offset]
                        .replace("\r\n", "\n")
                        .encode_utf16()
                        .count();
                    let editor_end = editor_start + before.encode_utf16().count();
                    let range = map.byte_range(editor_start..editor_end).unwrap();
                    assert_eq!(&source[range.clone()], before);
                    edits.push(edit(range.start, range.end, before, after));
                    expected = expected.replacen(before, after, 1);
                }
                assert_eq!(
                    batch(edits).apply(&source, 4096).unwrap().as_bytes(),
                    expected.as_bytes()
                );
            }
        }
    }
}

#[test]
fn front_matter_range_is_leading_fenced_and_byte_exact() {
    for (source, expected) in [
        (
            "\u{feff}---\r\ntitle: Café\r\n---\r\n# Start",
            Some("---\r\ntitle: Café\r\n---\r\n"),
        ),
        ("---\nlang: ja\n...\n## Start", Some("---\nlang: ja\n...\n")),
        ("---\nlang: ja\n---", Some("---\nlang: ja\n---")),
        ("# Start\n---\ntitle: Café\n---\n", None),
        ("---\n# Unclosed", None),
        ("---\nlang: ja\n---\r", None),
    ] {
        assert_eq!(
            front_matter_range(source).map(|range| &source[range]),
            expected
        );
    }
}

#[test]
fn source_edits_refuse_stale_text_unsafe_boundaries_and_wrong_line_endings() {
    let source = "\u{feff}A😀\r\n";
    for edits in [
        vec![edit(0, 3, "\u{feff}", "")],
        vec![edit(5, 8, "", "x")],
        vec![edit(9, 10, "\n", "")],
        vec![edit(4, 8, "another", "x")],
        vec![edit(4, 8, "😀", "x\n")],
        vec![edit(4, 8, "😀", "x\0")],
        vec![edit(4, 8, "😀", "x"), edit(3, 4, "A", "B")],
        vec![edit(3, 8, "A😀", "x"), edit(4, 8, "😀", "B")],
        vec![edit(4, 4, "", "x"), edit(4, 4, "", "y")],
    ] {
        assert!(batch(edits).apply(source, 4096).is_err());
    }
    assert_eq!(
        batch(vec![edit(4, 8, "😀", "本\r\n")])
            .apply(source, 4096)
            .unwrap(),
        "\u{feff}A本\r\n\r\n"
    );
    assert!(
        batch(vec![edit(0, 0, "", "\u{feff}")])
            .apply("plain", 4096)
            .is_err()
    );
    assert!(batch(vec![edit(0, 1, "a", "long")]).apply("a", 3).is_err());
    assert!(
        SourceEdits {
            profile: PROFILE_VERSION + 1,
            edits: vec![]
        }
        .apply(source, 4096)
        .is_err()
    );
}

#[test]
fn no_op_preserves_mixed_line_endings_and_source_with_opaque_spans() {
    let mixed = "\u{feff}---\r\nstatus: draft\n---\r\n<!-- untouched -->\n<div>opaque</div>\n";
    assert_eq!(batch(vec![]).apply(mixed, 4096).unwrap(), mixed);
    assert!(SourceMap::new(mixed).is_err());
    assert!(SourceMap::new("standalone\rline").is_err());
    assert!(
        batch(vec![edit(3, 3, "", "new")])
            .apply(mixed, 4096)
            .is_err()
    );
    assert!(batch(vec![]).apply("invalid\0", 4096).is_err());
}
