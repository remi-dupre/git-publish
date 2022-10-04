use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use git2::{Repository, Tree};

pub fn flatten_tree_with_prefix<'r>(
    repository: &'r Repository,
    tree: &Tree<'r>,
    prefix: &Path,
) -> impl Iterator<Item = Result<PathBuf>> {
    tree.into_iter().map(move |entry| {
        let name = prefix.join(entry.name().context("Invalid utf-8 in path")?);
        let obj = entry.to_object(repository)?;

        if let Some(tree) = obj.as_tree() {
        } else {
            todo!()
        }
    })
}

pub fn flatten_tree<'r>(
    repository: &'r Repository,
    tree: &Tree<'r>,
) -> impl Iterator<Item = Result<PathBuf>> {
    flatten_tree_with_prefix(repository, tree, Path::new(""))
}

fn diff_tree<'r>(tree_1: &Tree<'r>, tree_2: &Tree<'r>) -> impl Iterator<Item = PathBuf> {
    [].into_iter()
}
