use super::super::{descriptor, open};
use regex::Regex;
use std::{
    fs::File,
    io::{self, Read},
};

const RULE_FILE_LIMIT: u64 = 64 * 1024;
const MAX_RULES: usize = 512;

pub(super) struct DirectoryRules {
    prefix: String,
    git: Vec<Rule>,
    offish: Vec<Rule>,
}

impl DirectoryRules {
    pub(super) fn read(directory: &File, relative: &str) -> io::Result<Self> {
        Ok(Self {
            prefix: if relative.is_empty() {
                String::new()
            } else {
                format!("{relative}/")
            },
            git: read_rules(directory, ".gitignore")?,
            offish: read_rules(directory, ".offishignore")?,
        })
    }
}

pub(super) fn excluded(rules: &[DirectoryRules], path: &str, name: &str, directory: bool) -> bool {
    let mut git = false;
    let mut offish = false;
    for level in rules {
        let Some(relative) = path.strip_prefix(&level.prefix) else {
            continue;
        };
        for (rules, ignored) in [(&level.git, &mut git), (&level.offish, &mut offish)] {
            for rule in rules {
                let candidate = if rule.relative { relative } else { name };
                if (!rule.directory || directory) && rule.pattern.is_match(candidate) {
                    *ignored = !rule.negated;
                }
            }
        }
    }
    git || offish
}

fn read_rules(directory: &File, name: &str) -> io::Result<Vec<Rule>> {
    let file = match open(&descriptor(directory).join(name), false) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > RULE_FILE_LIMIT {
        return Err(io::Error::other(
            "Search exclusion file is not a small regular file.",
        ));
    }
    let mut contents = String::new();
    file.take(RULE_FILE_LIMIT + 1)
        .read_to_string(&mut contents)?;
    if contents.len() as u64 > RULE_FILE_LIMIT || contents.contains('\0') {
        return Err(io::Error::other(
            "Search exclusion file exceeds its limit or contains NUL.",
        ));
    }
    let contents = contents.strip_prefix('\u{feff}').unwrap_or(&contents);
    let rules: Vec<_> = contents
        .lines()
        .filter_map(Rule::parse)
        .take(MAX_RULES + 1)
        .collect();
    if rules.len() > MAX_RULES {
        return Err(io::Error::other(
            "Search exclusion file has too many rules.",
        ));
    }
    Ok(rules)
}

struct Rule {
    pattern: Regex,
    negated: bool,
    directory: bool,
    relative: bool,
}

impl Rule {
    fn parse(line: &str) -> Option<Self> {
        let mut line = line;
        while let Some(rest) = line.strip_suffix(' ') {
            if rest.bytes().rev().take_while(|byte| *byte == b'\\').count() % 2 == 1 {
                break;
            }
            line = rest;
        }
        if line.starts_with('#') || line.is_empty() {
            return None;
        }
        let negated = line.starts_with('!');
        let line = line.strip_prefix('!').unwrap_or(line);
        let directory = line.ends_with('/');
        let line = line.strip_suffix('/').unwrap_or(line);
        let relative = line.contains('/');
        let line = line.strip_prefix('/').unwrap_or(line);
        if line.is_empty() {
            return None;
        }
        Some(Self {
            pattern: glob_regex(line)?,
            negated,
            directory,
            relative,
        })
    }
}

fn glob_regex(glob: &str) -> Option<Regex> {
    let mut expression = String::from(r"\A");
    let mut chars = glob.chars().peekable();
    let mut component_start = true;
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => expression.push_str(&regex::escape(&chars.next()?.to_string())),
            '?' => expression.push_str("[^/]"),
            '*' => {
                let mut count = 1;
                while chars.next_if_eq(&'*').is_some() {
                    count += 1;
                }
                if count == 2 && component_start && chars.next_if_eq(&'/').is_some() {
                    expression.push_str("(?:[^/]+/)*");
                    continue;
                } else if count == 2 && component_start && chars.peek().is_none() {
                    expression.push_str("(?s:.*)");
                } else {
                    expression.push_str("[^/]*");
                }
            }
            '[' => {
                let mut class = String::from("[");
                if chars.next_if(|ch| matches!(ch, '!' | '^')).is_some() {
                    class.push('^');
                }
                if chars.next_if_eq(&']').is_some() {
                    class.push_str(r"\]");
                }
                loop {
                    match chars.next()? {
                        ']' => break,
                        '\\' => class.push_str(&regex::escape(&chars.next()?.to_string())),
                        '[' if chars.next_if_eq(&':').is_some() => {
                            class.push_str("[:");
                            loop {
                                let ch = chars.next()?;
                                class.push(ch);
                                if ch == ':' && chars.next_if_eq(&']').is_some() {
                                    class.push(']');
                                    break;
                                }
                            }
                        }
                        ch @ ('[' | '&' | '~') => {
                            class.push('\\');
                            class.push(ch);
                        }
                        ch => class.push(ch),
                    }
                }
                class.push_str("]&&[^/]");
                expression.push('[');
                expression.push_str(&class);
                expression.push(']');
            }
            _ => expression.push_str(&regex::escape(&ch.to_string())),
        }
        component_start = ch == '/';
    }
    expression.push_str(r"\z");
    Regex::new(&expression).ok()
}
