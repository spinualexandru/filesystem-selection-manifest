use std::{error::Error, fmt, ops::Range};

use pest::error::InputLocation;
use pest::{Parser, iterators::Pair};
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "fsman.pest"]
struct FsmanParser;

/// A parsed filesystem selection manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub entries: Vec<Entry>,
}

/// A single selection directive in a filesystem selection manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    Include { path: String },
    Exclude { path: String },
    IncludeChildren,
    IncludeRecursive,
    IncludeRecursiveAll,
    Descend { path: String, entries: Vec<Entry> },
}

/// An error encountered while parsing a filesystem selection manifest.
#[derive(Debug)]
pub struct ParseError(pest::error::Error<Rule>);

impl ParseError {
    /// Return the byte range in the source input where parsing failed.
    ///
    /// Point errors are represented by an empty range.
    pub fn byte_range(&self) -> Range<usize> {
        match self.0.location {
            InputLocation::Pos(position) => position..position,
            InputLocation::Span((start, end)) => start..end,
        }
    }

    /// Return the concise parser-generated error message.
    pub fn message(&self) -> String {
        self.0.variant.message().into_owned()
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Error for ParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

impl From<pest::error::Error<Rule>> for ParseError {
    fn from(error: pest::error::Error<Rule>) -> Self {
        Self(error)
    }
}

/// Parse a complete `.fsman` document into a typed manifest.
pub fn parse_manifest(input: &str) -> Result<Manifest, ParseError> {
    let mut pairs = FsmanParser::parse(Rule::manifest, input)?;
    let manifest = pairs
        .next()
        .expect("the manifest rule always produces one pair");

    Ok(Manifest {
        entries: parse_entries(manifest.into_inner()),
    })
}

fn parse_entries<'input>(pairs: impl Iterator<Item = Pair<'input, Rule>>) -> Vec<Entry> {
    pairs
        .filter(|pair| pair.as_rule() != Rule::EOI)
        .map(parse_entry)
        .collect()
}

fn parse_entry(pair: Pair<'_, Rule>) -> Entry {
    match pair.as_rule() {
        Rule::include => Entry::Include {
            path: pair
                .into_inner()
                .next()
                .expect("include rules contain a name")
                .as_str()
                .trim()
                .to_owned(),
        },
        Rule::exclude => Entry::Exclude {
            path: pair
                .into_inner()
                .next()
                .expect("exclude rules contain a path")
                .as_str()
                .trim()
                .to_owned(),
        },
        Rule::children => Entry::IncludeChildren,
        Rule::recursive => Entry::IncludeRecursive,
        Rule::recursive_all => Entry::IncludeRecursiveAll,
        Rule::block => {
            let mut inner = pair.into_inner();
            let path = inner
                .next()
                .expect("block rules contain a name")
                .as_str()
                .trim()
                .to_owned();

            Entry::Descend {
                path,
                entries: parse_entries(inner),
            }
        }
        rule => unreachable!("unexpected manifest rule: {rule:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_basic_example() {
        let manifest = parse_manifest(include_str!("../../../examples/basic.fsman")).unwrap();

        assert_eq!(
            manifest,
            Manifest {
                entries: vec![
                    Entry::Descend {
                        path: ".config".into(),
                        entries: vec![
                            Entry::Descend {
                                path: "ghostty".into(),
                                entries: vec![Entry::Descend {
                                    path: "themes".into(),
                                    entries: vec![Entry::Include {
                                        path: "noctalia".into(),
                                    }],
                                }],
                            },
                            Entry::Descend {
                                path: "fish".into(),
                                entries: vec![
                                    Entry::Include {
                                        path: "config.fish".into(),
                                    },
                                    Entry::Descend {
                                        path: "functions".into(),
                                        entries: vec![
                                            Entry::Include {
                                                path: "ff.fish".into(),
                                            },
                                            Entry::Include {
                                                path: "gitar.fish".into(),
                                            },
                                        ],
                                    },
                                ],
                            },
                            Entry::Descend {
                                path: "micro".into(),
                                entries: vec![Entry::Include {
                                    path: "settings.json".into(),
                                }],
                            },
                            Entry::Descend {
                                path: "hypr".into(),
                                entries: vec![
                                    Entry::IncludeRecursive,
                                    Entry::Exclude {
                                        path: "AGENTS.md".into(),
                                    },
                                ],
                            },
                            Entry::Descend {
                                path: "uwsm".into(),
                                entries: vec![Entry::Include { path: "env".into() }],
                            },
                            Entry::Descend {
                                path: "rog".into(),
                                entries: vec![Entry::Include {
                                    path: "rog-control-center.cfg".into(),
                                }],
                            },
                        ],
                    },
                    Entry::Descend {
                        path: "Work".into(),
                        entries: vec![
                            Entry::IncludeRecursive,
                            Entry::Descend {
                                path: "assets".into(),
                                entries: vec![Entry::IncludeRecursiveAll],
                            },
                        ],
                    },
                    Entry::Include {
                        path: ".vimrc".into(),
                    },
                    Entry::Include {
                        path: ".bashrc".into(),
                    },
                    Entry::Include {
                        path: ".hidden".into(),
                    },
                ],
            }
        );
    }

    #[test]
    fn parses_all_directives_and_names_with_spaces() {
        let manifest = parse_manifest(
            "plain file\n!excluded path\n*\n**\n***\nfolder name {\n  nested file\n}\n",
        )
        .unwrap();

        assert_eq!(
            manifest.entries,
            vec![
                Entry::Include {
                    path: "plain file".into(),
                },
                Entry::Exclude {
                    path: "excluded path".into(),
                },
                Entry::IncludeChildren,
                Entry::IncludeRecursive,
                Entry::IncludeRecursiveAll,
                Entry::Descend {
                    path: "folder name".into(),
                    entries: vec![Entry::Include {
                        path: "nested file".into(),
                    }],
                },
            ]
        );
    }

    #[test]
    fn accepts_blank_lines_tabs_crlf_and_no_final_newline() {
        let manifest =
            parse_manifest("\r\n\tdir {\r\n\t\tfile name\r\n\t}\r\n\r\n.hidden").unwrap();

        assert_eq!(
            manifest.entries,
            vec![
                Entry::Descend {
                    path: "dir".into(),
                    entries: vec![Entry::Include {
                        path: "file name".into(),
                    }],
                },
                Entry::Include {
                    path: ".hidden".into(),
                },
            ]
        );
    }

    #[test]
    fn rejects_invalid_syntax() {
        for invalid in [
            "!\n",
            "!  \t\n",
            "}\n",
            "dir {\nfile\n",
            "{\n}\n",
            "!dir {\n}\n",
            "dir { file\n}\n",
            "name } trailing\n",
        ] {
            assert!(
                parse_manifest(invalid).is_err(),
                "expected parsing to fail for {invalid:?}"
            );
        }
    }

    #[test]
    fn exposes_structured_parse_error_details() {
        let error = parse_manifest("dir {\nfile\n").unwrap_err();

        assert_eq!(error.byte_range(), 11..11);
        assert!(!error.message().is_empty());
    }
}
