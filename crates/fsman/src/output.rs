use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use crate::{Entry, Manifest, ResolvedEntry, ResolvedEntryKind, ResolvedTree};

/// A terminal or collapsed path selected from a resolved tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputPath {
    /// The full path to the selected filesystem entry.
    pub path: PathBuf,
    /// The kind of filesystem entry that was selected.
    pub kind: ResolvedEntryKind,
}

/// Select output paths from a resolved tree.
///
/// When `short` is false, the result contains terminal entries. When it is
/// true, complete recursive selections are collapsed to their highest selected
/// directory. The manifest and recursion depth must match those used to create
/// `tree`.
pub fn output_paths(
    tree: &ResolvedTree,
    manifest: &Manifest,
    recursive_depth: Option<usize>,
    short: bool,
) -> Vec<OutputPath> {
    if short {
        let (root_collapsed, paths) = short_paths(tree, manifest, recursive_depth);
        if root_collapsed {
            return vec![OutputPath {
                path: tree.root.clone(),
                kind: ResolvedEntryKind::Directory,
            }];
        }
        paths
            .into_iter()
            .map(|entry| OutputPath {
                path: tree.root.join(entry.path),
                kind: entry.kind,
            })
            .collect()
    } else {
        let mut paths = Vec::new();
        collect_terminal_paths(&tree.entries, &mut paths);
        paths
    }
}

fn collect_recursive_directories(
    entries: &[Entry],
    directory: &Path,
    recursive_depth: Option<usize>,
    collapsed: &mut BTreeSet<PathBuf>,
    expanded: &mut BTreeSet<PathBuf>,
    git_filtered: &mut BTreeSet<PathBuf>,
) {
    let selects_all_recursively = entries
        .iter()
        .any(|entry| matches!(entry, Entry::IncludeRecursiveAll));
    let selects_git_filtered_recursively = entries
        .iter()
        .any(|entry| matches!(entry, Entry::IncludeRecursive));
    let has_exclusions = entries
        .iter()
        .any(|entry| matches!(entry, Entry::Exclude { .. }));
    if selects_all_recursively {
        if recursive_depth.is_none() && !has_exclusions {
            collapsed.insert(directory.to_path_buf());
        } else {
            expanded.insert(directory.to_path_buf());
        }
        return;
    }
    if selects_git_filtered_recursively {
        git_filtered.insert(directory.to_path_buf());
        return;
    }

    for entry in entries {
        if let Entry::Descend { path, entries } = entry {
            collect_recursive_directories(
                entries,
                &directory.join(path),
                recursive_depth,
                collapsed,
                expanded,
                git_filtered,
            );
        }
    }
}

struct ShortPath {
    path: PathBuf,
    kind: ResolvedEntryKind,
}

fn short_paths(
    tree: &ResolvedTree,
    manifest: &Manifest,
    recursive_depth: Option<usize>,
) -> (bool, Vec<ShortPath>) {
    let mut collapsed_directories = BTreeSet::new();
    let mut expanded_directories = BTreeSet::new();
    let mut git_filtered_directories = BTreeSet::new();
    collect_recursive_directories(
        &manifest.entries,
        Path::new(""),
        recursive_depth,
        &mut collapsed_directories,
        &mut expanded_directories,
        &mut git_filtered_directories,
    );
    if collapsed_directories.contains(Path::new("")) {
        return (true, Vec::new());
    }

    let mut paths = Vec::new();
    if git_filtered_directories.contains(Path::new("")) {
        collect_terminal_short_paths(&tree.root, &tree.entries, &mut paths);
    } else if expanded_directories.contains(Path::new("")) {
        collect_immediate_paths(&tree.root, &tree.entries, &mut paths);
    } else {
        collect_short_paths(
            &tree.root,
            &tree.entries,
            &collapsed_directories,
            &expanded_directories,
            &git_filtered_directories,
            &mut paths,
        );
    }
    (false, paths)
}

fn collect_short_paths(
    root: &Path,
    entries: &[ResolvedEntry],
    collapsed_directories: &BTreeSet<PathBuf>,
    expanded_directories: &BTreeSet<PathBuf>,
    git_filtered_directories: &BTreeSet<PathBuf>,
    paths: &mut Vec<ShortPath>,
) {
    for entry in entries {
        let relative = entry
            .path
            .strip_prefix(root)
            .expect("resolved entries are always below the resolution root");
        if git_filtered_directories.contains(relative) {
            collect_terminal_short_paths(root, std::slice::from_ref(entry), paths);
        } else if expanded_directories.contains(relative) {
            if entry.children.is_empty() {
                paths.push(ShortPath {
                    path: relative.to_path_buf(),
                    kind: entry.kind,
                });
            } else {
                collect_immediate_paths(root, &entry.children, paths);
            }
        } else if entry.children.is_empty() || collapsed_directories.contains(relative) {
            paths.push(ShortPath {
                path: relative.to_path_buf(),
                kind: entry.kind,
            });
        } else {
            collect_short_paths(
                root,
                &entry.children,
                collapsed_directories,
                expanded_directories,
                git_filtered_directories,
                paths,
            );
        }
    }
}

fn collect_terminal_short_paths(
    root: &Path,
    entries: &[ResolvedEntry],
    paths: &mut Vec<ShortPath>,
) {
    for entry in entries {
        if entry.children.is_empty() {
            paths.push(ShortPath {
                path: entry
                    .path
                    .strip_prefix(root)
                    .expect("resolved entries are always below the resolution root")
                    .to_path_buf(),
                kind: entry.kind,
            });
        } else {
            collect_terminal_short_paths(root, &entry.children, paths);
        }
    }
}

fn collect_immediate_paths(root: &Path, entries: &[ResolvedEntry], paths: &mut Vec<ShortPath>) {
    paths.extend(entries.iter().map(|entry| {
        ShortPath {
            path: entry
                .path
                .strip_prefix(root)
                .expect("resolved entries are always below the resolution root")
                .to_path_buf(),
            kind: entry.kind,
        }
    }));
}

fn collect_terminal_paths(entries: &[ResolvedEntry], paths: &mut Vec<OutputPath>) {
    for entry in entries {
        if entry.children.is_empty() {
            paths.push(OutputPath {
                path: entry.path.clone(),
                kind: entry.kind,
            });
        } else {
            collect_terminal_paths(&entry.children, paths);
        }
    }
}
