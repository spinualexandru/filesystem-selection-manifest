use std::{
    env,
    error::Error,
    ffi::{OsStr, OsString},
    fmt, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), CliError> {
    match parse_arguments()? {
        Command::Validate { manifest_path } => {
            read_manifest(&manifest_path)?;
            println!("{} is valid", manifest_path.display());
        }
        Command::Resolve {
            manifest_path,
            cwd,
            depth,
            short,
            json,
            flat,
        } => {
            ensure_directory(&cwd)?;
            let manifest = read_manifest(&manifest_path)?;
            let tree = fsman::resolve_manifest(&manifest, &cwd, depth).map_err(|source| {
                CliError::Resolve {
                    cwd: cwd.clone(),
                    source,
                }
            })?;
            if json && flat {
                print_flat_json(&tree, &manifest, depth, true);
            } else if json {
                print_json(&tree, &manifest, depth, short);
            } else if flat {
                print_flat(&tree, &manifest, depth, true);
            } else if short {
                print_short_tree(&tree, &manifest, depth);
            } else {
                print_tree(&tree);
            }
        }
    }

    Ok(())
}

enum Command {
    Validate {
        manifest_path: PathBuf,
    },
    Resolve {
        manifest_path: PathBuf,
        cwd: PathBuf,
        depth: Option<usize>,
        short: bool,
        json: bool,
        flat: bool,
    },
}

fn parse_arguments() -> Result<Command, CliError> {
    let mut arguments = env::args_os().skip(1);
    let first = arguments.next().ok_or(CliError::Usage)?;

    if first == OsStr::new("resolve") {
        parse_resolve_arguments(arguments)
    } else if first == OsStr::new("validate") {
        let manifest_path = arguments.next().ok_or(CliError::Usage)?;
        if arguments.next().is_some() {
            return Err(CliError::Usage);
        }
        Ok(Command::Validate {
            manifest_path: manifest_path.into(),
        })
    } else {
        if arguments.next().is_some() {
            return Err(CliError::Usage);
        }
        Ok(Command::Validate {
            manifest_path: first.into(),
        })
    }
}

fn parse_resolve_arguments(
    mut arguments: impl Iterator<Item = OsString>,
) -> Result<Command, CliError> {
    let manifest_path = PathBuf::from(arguments.next().ok_or(CliError::Usage)?);
    let mut cwd = None;
    let mut depth = None;
    let mut short = false;
    let mut json = false;
    let mut flat = false;

    while let Some(argument) = arguments.next() {
        if argument == OsStr::new("--cwd") && cwd.is_none() {
            cwd = Some(PathBuf::from(arguments.next().ok_or(CliError::Usage)?));
        } else if argument == OsStr::new("--depth") && depth.is_none() {
            let value = arguments.next().ok_or(CliError::Usage)?;
            let parsed = value
                .to_str()
                .and_then(|value| value.parse::<usize>().ok())
                .ok_or_else(|| CliError::InvalidDepth {
                    value: value.clone(),
                })?;
            depth = Some(parsed);
        } else if argument == OsStr::new("--short") && !short {
            short = true;
        } else if argument == OsStr::new("--json") && !json {
            json = true;
        } else if argument == OsStr::new("--flat") && !flat {
            flat = true;
        } else {
            return Err(CliError::Usage);
        }
    }

    let cwd = match cwd {
        Some(cwd) => cwd,
        None => env::current_dir().map_err(CliError::CurrentDirectory)?,
    };
    Ok(Command::Resolve {
        manifest_path,
        cwd,
        depth,
        short,
        json,
        flat,
    })
}

fn read_manifest(path: &Path) -> Result<fsman::Manifest, CliError> {
    let contents = fs::read_to_string(path).map_err(|source| CliError::Read {
        path: path.to_path_buf(),
        source,
    })?;

    fsman::parse_manifest(&contents).map_err(|source| CliError::Parse {
        path: path.to_path_buf(),
        source: Box::new(source),
    })
}

fn ensure_directory(path: &Path) -> Result<(), CliError> {
    let metadata = fs::metadata(path).map_err(|source| CliError::Cwd {
        path: path.to_path_buf(),
        source,
    })?;
    if !metadata.is_dir() {
        return Err(CliError::Cwd {
            path: path.to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::NotADirectory,
                "path is not a directory",
            ),
        });
    }
    Ok(())
}

fn print_tree(tree: &fsman::ResolvedTree) {
    println!("{}", tree.root.display());
    print_entries(&tree.entries, "");
}

fn print_short_tree(
    tree: &fsman::ResolvedTree,
    manifest: &fsman::Manifest,
    recursive_depth: Option<usize>,
) {
    print_short_root(&tree.root);
    let paths = fsman::output_paths(tree, manifest, recursive_depth, true)
        .into_iter()
        .filter(|entry| entry.path != tree.root)
        .collect::<Vec<_>>();
    for (index, entry) in paths.iter().enumerate() {
        let connector = if index + 1 == paths.len() {
            "└── "
        } else {
            "├── "
        };
        let suffix = if entry.kind == fsman::ResolvedEntryKind::Directory {
            std::path::MAIN_SEPARATOR_STR
        } else {
            ""
        };
        let relative = entry
            .path
            .strip_prefix(&tree.root)
            .expect("output paths are always below the resolution root");
        println!("{connector}{}{suffix}", relative.display());
    }
}

fn print_short_root(root: &Path) {
    let display = root.display().to_string();
    if display.ends_with(std::path::MAIN_SEPARATOR) {
        println!("{display}");
    } else {
        println!("{display}{}", std::path::MAIN_SEPARATOR);
    }
}

fn print_flat(
    tree: &fsman::ResolvedTree,
    manifest: &fsman::Manifest,
    recursive_depth: Option<usize>,
    short: bool,
) {
    for entry in fsman::output_paths(tree, manifest, recursive_depth, short) {
        println!("{}", formatted_output_path(&entry));
    }
}

fn print_flat_json(
    tree: &fsman::ResolvedTree,
    manifest: &fsman::Manifest,
    recursive_depth: Option<usize>,
    short: bool,
) {
    let paths = fsman::output_paths(tree, manifest, recursive_depth, short)
        .iter()
        .map(formatted_output_path)
        .collect::<Vec<_>>();
    println!(
        "{}",
        serde_json::to_string_pretty(&paths).expect("filesystem paths always serialize")
    );
}

fn formatted_output_path(entry: &fsman::OutputPath) -> String {
    let mut path = entry.path.display().to_string();
    if entry.kind == fsman::ResolvedEntryKind::Directory
        && !path.ends_with(std::path::MAIN_SEPARATOR)
    {
        path.push(std::path::MAIN_SEPARATOR);
    }
    path
}

fn print_json(
    tree: &fsman::ResolvedTree,
    manifest: &fsman::Manifest,
    recursive_depth: Option<usize>,
    short: bool,
) {
    let value = if short {
        let paths = fsman::output_paths(tree, manifest, recursive_depth, true);
        serde_json::json!({
            "path": tree.root.to_string_lossy(),
            "type": "directory",
            "children": paths
                .iter()
                .filter(|entry| entry.path != tree.root)
                .map(|entry| serde_json::json!({
                    "path": entry.path.to_string_lossy(),
                    "type": kind_name(entry.kind),
                    "children": [],
                }))
                .collect::<Vec<_>>(),
        })
    } else {
        serde_json::json!({
            "path": tree.root.to_string_lossy(),
            "type": "directory",
            "children": tree.entries.iter().map(entry_json).collect::<Vec<_>>(),
        })
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&value).expect("resolved trees always serialize")
    );
}

fn entry_json(entry: &fsman::ResolvedEntry) -> serde_json::Value {
    serde_json::json!({
        "path": entry.path.to_string_lossy(),
        "type": kind_name(entry.kind),
        "children": entry.children.iter().map(entry_json).collect::<Vec<_>>(),
    })
}

fn kind_name(kind: fsman::ResolvedEntryKind) -> &'static str {
    match kind {
        fsman::ResolvedEntryKind::File => "file",
        fsman::ResolvedEntryKind::Directory => "directory",
        fsman::ResolvedEntryKind::Symlink => "symlink",
        fsman::ResolvedEntryKind::Other => "other",
    }
}

fn print_entries(entries: &[fsman::ResolvedEntry], prefix: &str) {
    for (index, entry) in entries.iter().enumerate() {
        let is_last = index + 1 == entries.len();
        let connector = if is_last { "└── " } else { "├── " };
        let name = entry
            .path
            .file_name()
            .expect("resolved entries always have a file name")
            .to_string_lossy();
        println!("{prefix}{connector}{name}");

        let child_prefix = if is_last {
            format!("{prefix}    ")
        } else {
            format!("{prefix}│   ")
        };
        print_entries(&entry.children, &child_prefix);
    }
}

#[derive(Debug)]
enum CliError {
    Usage,
    InvalidDepth {
        value: OsString,
    },
    CurrentDirectory(std::io::Error),
    Cwd {
        path: PathBuf,
        source: std::io::Error,
    },
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse {
        path: PathBuf,
        source: Box<fsman::ParseError>,
    },
    Resolve {
        cwd: PathBuf,
        source: fsman::ResolveError,
    },
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage => write!(
                formatter,
                "usage: fsman-cli <FILE>\n       fsman-cli validate <FILE>\n       fsman-cli resolve <FILE> [--cwd <DIRECTORY>] [--depth <LEVELS>] [--short] [--json] [--flat]"
            ),
            Self::InvalidDepth { value } => write!(
                formatter,
                "invalid depth {:?}: expected a non-negative integer",
                value
            ),
            Self::CurrentDirectory(source) => {
                write!(
                    formatter,
                    "could not determine the current directory: {source}"
                )
            }
            Self::Cwd { path, source } => {
                write!(
                    formatter,
                    "could not use {} as the working directory: {source}",
                    path.display()
                )
            }
            Self::Read { path, source } => {
                write!(formatter, "could not read {}: {source}", path.display())
            }
            Self::Parse { path, source } => {
                write!(
                    formatter,
                    "{} is not a valid fsman file:\n{source}",
                    path.display()
                )
            }
            Self::Resolve { cwd, source } => {
                write!(
                    formatter,
                    "could not resolve against {}: {source}",
                    cwd.display()
                )
            }
        }
    }
}

impl Error for CliError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Usage | Self::InvalidDepth { .. } => None,
            Self::CurrentDirectory(source) => Some(source),
            Self::Cwd { source, .. } | Self::Read { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
            Self::Resolve { source, .. } => Some(source),
        }
    }
}
