use pulldown_cmark::{Event, Options, Parser, Tag};
use std::{env, fs};

fn main() {
    let source = fs::read_to_string(env::args_os().nth(1).expect("fixture path")).unwrap();
    let old = "Keep the first release on the current schedule.";
    let new = "Move the pilot launch to 14 December.";
    let matches: Vec<_> = Parser::new_ext(&source, Options::ENABLE_TABLES)
        .into_offset_iter()
        .filter_map(|(event, range)| match event {
            Event::Start(Tag::Paragraph) if source[range.clone()].trim_end_matches('\n') == old => {
                Some(range.start..range.start + old.len())
            }
            _ => None,
        })
        .collect();
    let [span] = matches.as_slice() else { panic!("expected one supported source span") };
    let result = format!("{}{}{}", &source[..span.start], new, &source[span.end..]);
    assert_eq!(result, source.replacen(old, new, 1));
    assert!(Parser::new_ext(&result, Options::ENABLE_TABLES).any(|event| {
        matches!(event, Event::Start(Tag::Table(_)))
    }));
    println!("PASS: Rust parser supplies a validated UTF-8 span and recognises the table");
}
