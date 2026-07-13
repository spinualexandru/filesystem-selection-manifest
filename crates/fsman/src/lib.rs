mod parser;
mod resolver;

pub use parser::{Entry, Manifest, ParseError, parse_manifest};
pub use resolver::{
    ResolveError, ResolvedEntry, ResolvedEntryKind, ResolvedTree, resolve_manifest,
};
