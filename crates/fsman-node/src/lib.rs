use serde_json::{Value, json};

#[neon::export(task)]
fn validate(source: String) -> bool {
    fsman::parse_manifest(&source).is_ok()
}

#[neon::export(task)]
fn resolve(
    source: String,
    root: String,
    recursive_depth: Option<f64>,
    short: bool,
    flat: bool,
) -> Result<String, String> {
    let recursive_depth = match recursive_depth {
        Some(depth)
            if depth.is_finite()
                && depth >= 0.0
                && depth.fract() == 0.0
                && depth <= usize::MAX as f64 =>
        {
            Some(depth as usize)
        }
        Some(_) => return Err("depth must be a non-negative integer".into()),
        None => None,
    };
    let manifest = fsman::parse_manifest(&source).map_err(|error| error.to_string())?;
    let tree = fsman::resolve_manifest(&manifest, &root, recursive_depth)
        .map_err(|error| error.to_string())?;
    let value = if flat {
        Value::Array(
            fsman::output_paths(&tree, &manifest, recursive_depth, short)
                .iter()
                .map(|entry| Value::String(formatted_output_path(entry)))
                .collect(),
        )
    } else if short {
        let children = fsman::output_paths(&tree, &manifest, recursive_depth, true)
            .iter()
            .filter(|entry| entry.path != tree.root)
            .map(|entry| {
                json!({
                    "path": entry.path.to_string_lossy(),
                    "type": kind_name(entry.kind),
                    "children": [],
                })
            })
            .collect::<Vec<_>>();
        json!({
            "path": tree.root.to_string_lossy(),
            "type": "directory",
            "children": children,
        })
    } else {
        json!({
            "path": tree.root.to_string_lossy(),
            "type": "directory",
            "children": tree.entries.iter().map(entry_json).collect::<Vec<_>>(),
        })
    };

    serde_json::to_string(&value).map_err(|error| error.to_string())
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

fn entry_json(entry: &fsman::ResolvedEntry) -> Value {
    json!({
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

#[neon::main]
fn main(mut context: neon::context::ModuleContext) -> neon::result::NeonResult<()> {
    neon::registered().export(&mut context)?;
    Ok(())
}
