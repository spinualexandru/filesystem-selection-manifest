use std::{
    collections::BTreeMap,
    error::Error,
    ffi::{OsStr, OsString},
    fmt, fs, io,
    path::{Component, Path, PathBuf},
};

use crate::{Entry, Manifest};

/// The result of resolving a manifest against a directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTree {
    /// The directory against which the manifest was resolved.
    pub root: PathBuf,
    /// The selected filesystem entries below [`Self::root`].
    pub entries: Vec<ResolvedEntry>,
}

/// A selected filesystem entry and any selected children below it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedEntry {
    /// The full path to this entry.
    pub path: PathBuf,
    /// The kind of filesystem entry that was found.
    pub kind: ResolvedEntryKind,
    /// Selected children. This is empty for files and unexpanded directories.
    pub children: Vec<ResolvedEntry>,
}

/// The kind of a resolved filesystem entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedEntryKind {
    File,
    Directory,
    Symlink,
    Other,
}

/// An error encountered while resolving a manifest against the filesystem.
#[derive(Debug)]
pub enum ResolveError {
    InvalidManifestPath { path: String },
    ReadDirectory { path: PathBuf, source: io::Error },
    ReadMetadata { path: PathBuf, source: io::Error },
}

impl fmt::Display for ResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidManifestPath { path } => write!(
                formatter,
                "manifest path {path:?} must be relative and must not contain `..`"
            ),
            Self::ReadDirectory { path, source } => {
                write!(
                    formatter,
                    "could not read directory {}: {source}",
                    path.display()
                )
            }
            Self::ReadMetadata { path, source } => {
                write!(formatter, "could not inspect {}: {source}", path.display())
            }
        }
    }
}

impl Error for ResolveError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidManifestPath { .. } => None,
            Self::ReadDirectory { source, .. } | Self::ReadMetadata { source, .. } => Some(source),
        }
    }
}

/// Resolve a parsed manifest against `root`.
///
/// Literal paths that do not exist are omitted. `recursive_depth` limits how
/// many levels each `**` directive traverses; `None` permits unlimited
/// traversal. Symbolic links are selected but are never followed.
pub fn resolve_manifest(
    manifest: &Manifest,
    root: impl AsRef<Path>,
    recursive_depth: Option<usize>,
) -> Result<ResolvedTree, ResolveError> {
    let root = root.as_ref().to_path_buf();
    let entries = resolve_entries(&root, &manifest.entries, recursive_depth)?;

    Ok(ResolvedTree {
        root,
        entries: entries.into_values().collect(),
    })
}

fn resolve_entries(
    directory: &Path,
    directives: &[Entry],
    recursive_depth: Option<usize>,
) -> Result<BTreeMap<OsString, ResolvedEntry>, ResolveError> {
    let exclusions = directives
        .iter()
        .filter_map(|entry| match entry {
            Entry::Exclude { path } => Some(validate_relative_path(path)),
            _ => None,
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut resolved = BTreeMap::new();

    for directive in directives {
        match directive {
            Entry::Include { path } => {
                let relative = validate_relative_path(path)?;
                if !is_excluded(&relative, &exclusions) {
                    insert_literal(&mut resolved, directory, &relative, None)?;
                }
            }
            Entry::Exclude { .. } => {}
            Entry::IncludeChildren => {
                for (name, entry) in read_children(directory)? {
                    let relative = Path::new(&name);
                    if !is_excluded(relative, &exclusions) {
                        merge_entry(&mut resolved, name, entry);
                    }
                }
            }
            Entry::IncludeRecursive => {
                if recursive_depth != Some(0) {
                    for (name, entry) in resolve_recursive(
                        directory,
                        Path::new(""),
                        1,
                        recursive_depth,
                        &exclusions,
                    )? {
                        merge_entry(&mut resolved, name, entry);
                    }
                }
            }
            Entry::Descend { path, entries } => {
                let relative = validate_relative_path(path)?;
                if is_excluded(&relative, &exclusions) {
                    continue;
                }

                let full_path = directory.join(&relative);
                let Some(kind) = entry_kind_if_present(&full_path)? else {
                    continue;
                };
                if kind != ResolvedEntryKind::Directory {
                    continue;
                }

                let children = resolve_entries(&full_path, entries, recursive_depth)?
                    .into_values()
                    .collect();
                insert_literal(&mut resolved, directory, &relative, Some(children))?;
            }
        }
    }

    Ok(resolved)
}

fn resolve_recursive(
    directory: &Path,
    relative_directory: &Path,
    level: usize,
    max_depth: Option<usize>,
    exclusions: &[PathBuf],
) -> Result<BTreeMap<OsString, ResolvedEntry>, ResolveError> {
    let mut resolved = BTreeMap::new();

    for (name, mut entry) in read_children(directory)? {
        let relative = relative_directory.join(&name);
        if is_excluded(&relative, exclusions) {
            continue;
        }

        if entry.kind == ResolvedEntryKind::Directory && max_depth.is_none_or(|depth| level < depth)
        {
            entry.children =
                resolve_recursive(&entry.path, &relative, level + 1, max_depth, exclusions)?
                    .into_values()
                    .collect();
        }
        resolved.insert(name, entry);
    }

    Ok(resolved)
}

fn read_children(directory: &Path) -> Result<BTreeMap<OsString, ResolvedEntry>, ResolveError> {
    let children = fs::read_dir(directory).map_err(|source| ResolveError::ReadDirectory {
        path: directory.to_path_buf(),
        source,
    })?;
    let mut resolved = BTreeMap::new();

    for child in children {
        let child = child.map_err(|source| ResolveError::ReadDirectory {
            path: directory.to_path_buf(),
            source,
        })?;
        let path = child.path();
        let kind = entry_kind(&path)?;
        let name = child.file_name();
        resolved.insert(
            name,
            ResolvedEntry {
                path,
                kind,
                children: Vec::new(),
            },
        );
    }

    Ok(resolved)
}

fn insert_literal(
    entries: &mut BTreeMap<OsString, ResolvedEntry>,
    directory: &Path,
    relative: &Path,
    final_children: Option<Vec<ResolvedEntry>>,
) -> Result<(), ResolveError> {
    let components = relative
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name.to_os_string()),
            Component::CurDir => None,
            _ => unreachable!("manifest paths are validated before insertion"),
        })
        .collect::<Vec<_>>();
    insert_literal_components(entries, directory, &components, final_children)
}

fn insert_literal_components(
    entries: &mut BTreeMap<OsString, ResolvedEntry>,
    directory: &Path,
    components: &[OsString],
    final_children: Option<Vec<ResolvedEntry>>,
) -> Result<(), ResolveError> {
    let Some((name, remaining)) = components.split_first() else {
        return Ok(());
    };
    let path = directory.join(name);
    let Some(kind) = entry_kind_if_present(&path)? else {
        return Ok(());
    };
    let entry = entries
        .entry(name.clone())
        .or_insert_with(|| ResolvedEntry {
            path: path.clone(),
            kind,
            children: Vec::new(),
        });

    if remaining.is_empty() {
        if let Some(children) = final_children {
            merge_children(&mut entry.children, children);
        }
        return Ok(());
    }
    if kind != ResolvedEntryKind::Directory {
        return Ok(());
    }

    let mut children = std::mem::take(&mut entry.children)
        .into_iter()
        .map(|child| (entry_name(&child).to_os_string(), child))
        .collect();
    insert_literal_components(&mut children, &path, remaining, final_children)?;
    entry.children = children.into_values().collect();
    Ok(())
}

fn merge_entry(
    entries: &mut BTreeMap<OsString, ResolvedEntry>,
    name: OsString,
    entry: ResolvedEntry,
) {
    match entries.get_mut(&name) {
        Some(existing) => merge_children(&mut existing.children, entry.children),
        None => {
            entries.insert(name, entry);
        }
    }
}

fn merge_children(existing: &mut Vec<ResolvedEntry>, additional: Vec<ResolvedEntry>) {
    let mut merged = existing
        .drain(..)
        .map(|entry| (entry_name(&entry).to_os_string(), entry))
        .collect::<BTreeMap<_, _>>();
    for entry in additional {
        merge_entry(&mut merged, entry_name(&entry).to_os_string(), entry);
    }
    *existing = merged.into_values().collect();
}

fn entry_name(entry: &ResolvedEntry) -> &OsStr {
    entry
        .path
        .file_name()
        .expect("resolved entries always have a file name")
}

fn entry_kind_if_present(path: &Path) -> Result<Option<ResolvedEntryKind>, ResolveError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(kind_from_file_type(metadata.file_type()))),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(ResolveError::ReadMetadata {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn entry_kind(path: &Path) -> Result<ResolvedEntryKind, ResolveError> {
    entry_kind_if_present(path)?.ok_or_else(|| ResolveError::ReadMetadata {
        path: path.to_path_buf(),
        source: io::Error::new(io::ErrorKind::NotFound, "entry disappeared while resolving"),
    })
}

fn kind_from_file_type(file_type: fs::FileType) -> ResolvedEntryKind {
    if file_type.is_file() {
        ResolvedEntryKind::File
    } else if file_type.is_dir() {
        ResolvedEntryKind::Directory
    } else if file_type.is_symlink() {
        ResolvedEntryKind::Symlink
    } else {
        ResolvedEntryKind::Other
    }
}

fn validate_relative_path(path: &str) -> Result<PathBuf, ResolveError> {
    let mut normalized = PathBuf::new();
    for component in Path::new(path).components() {
        match component {
            Component::Normal(name) => normalized.push(name),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(ResolveError::InvalidManifestPath {
                    path: path.to_owned(),
                });
            }
        }
    }

    (!normalized.as_os_str().is_empty())
        .then_some(normalized)
        .ok_or_else(|| ResolveError::InvalidManifestPath {
            path: path.to_owned(),
        })
}

fn is_excluded(path: &Path, exclusions: &[PathBuf]) -> bool {
    exclusions
        .iter()
        .any(|excluded| path == excluded || path.starts_with(excluded))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use super::*;
    use crate::parse_manifest;

    static NEXT_TEMP_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let unique = NEXT_TEMP_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("fsman-resolver-{}-{unique}", std::process::id()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn create_file(&self, relative: &str) {
            let path = self.0.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, "test").unwrap();
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn relative_paths(tree: &ResolvedTree) -> Vec<PathBuf> {
        fn collect(root: &Path, entries: &[ResolvedEntry], paths: &mut Vec<PathBuf>) {
            for entry in entries {
                paths.push(entry.path.strip_prefix(root).unwrap().to_path_buf());
                collect(root, &entry.children, paths);
            }
        }

        let mut paths = Vec::new();
        collect(&tree.root, &tree.entries, &mut paths);
        paths
    }

    #[test]
    fn resolves_literals_children_and_exclusions() {
        let directory = TestDirectory::new();
        directory.create_file("literal.txt");
        directory.create_file("folder/keep.txt");
        directory.create_file("folder/skip.txt");
        let manifest =
            parse_manifest("literal.txt\nmissing.txt\nfolder {\n  *\n  !skip.txt\n}\n").unwrap();

        let tree = resolve_manifest(&manifest, &directory.0, None).unwrap();

        assert_eq!(
            relative_paths(&tree),
            ["folder", "folder/keep.txt", "literal.txt"]
                .map(PathBuf::from)
                .to_vec()
        );
    }

    #[test]
    fn limits_each_recursive_directive_to_the_requested_depth() {
        let directory = TestDirectory::new();
        directory.create_file("one/two/three/file.txt");
        let manifest = parse_manifest("**\n").unwrap();

        let tree = resolve_manifest(&manifest, &directory.0, Some(2)).unwrap();

        assert_eq!(
            relative_paths(&tree),
            ["one", "one/two"].map(PathBuf::from).to_vec()
        );
    }

    #[test]
    fn unlimited_recursion_visits_all_descendants() {
        let directory = TestDirectory::new();
        directory.create_file("one/two/three/file.txt");
        let manifest = parse_manifest("**\n").unwrap();

        let tree = resolve_manifest(&manifest, &directory.0, None).unwrap();

        assert_eq!(
            relative_paths(&tree),
            ["one", "one/two", "one/two/three", "one/two/three/file.txt"]
                .map(PathBuf::from)
                .to_vec()
        );
    }

    #[test]
    fn rejects_paths_that_escape_the_resolution_root() {
        let manifest = parse_manifest("../secret\n").unwrap();

        assert!(matches!(
            resolve_manifest(&manifest, ".", None),
            Err(ResolveError::InvalidManifestPath { .. })
        ));
    }
}
