use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use git2::{Commit, Oid, Repository, Tree};

pub struct Rebuilder<'c, 'r> {
    repository: &'r Repository,
    include: &'c [PathBuf],
    exclude: &'c [PathBuf],

    /// Map of required objects IDs (old_id -> new_id)
    id_map: HashMap<Oid, Oid>,

    // TODO: compute this without this structure from git + id_map
    /// Keep track of paths that have been filtered-out
    filtered: HashSet<PathBuf>,
}

impl<'c, 'r> Rebuilder<'c, 'r> {
    pub fn new(repository: &'r Repository, include: &'c [PathBuf], exclude: &'c [PathBuf]) -> Self {
        Self {
            repository,
            id_map: HashMap::new(),
            include,
            exclude,
            filtered: HashSet::new(),
        }
    }

    pub fn debug_filtered(&self) {
        if self.filtered.is_empty() {
            println!("All files were kept");
        } else {
            println!("Filtered {} paths:", self.filtered.len());
            let mut sorted_vec: Vec<_> = self.filtered.iter().collect();
            sorted_vec.sort_unstable();

            for path in sorted_vec {
                println!("  - {}", path.display());
            }
        }
    }

    pub fn debug_changes(&self) {
        let changed = (self.id_map).iter().filter(|(k, v)| k != v).count();
        let unchanged = self.id_map.len() - changed;
        println!("Rewrote {changed} objects and skipped {unchanged}");
    }

    pub fn changed(&self, id: Oid) -> bool {
        self.id_map.get(&id).unwrap_or(&id) != &id
    }

    pub fn rebuild_tree(&mut self, tree: &Tree<'r>, prefix: &Path) -> Result<Tree<'r>> {
        let new_tree_id = {
            if let Some(cached) = self.id_map.get(&tree.id()) {
                *cached
            } else {
                let mut changed = false;
                let mut builder = self.repository.treebuilder(None)?;

                for entry in tree {
                    let entry_name = entry.name().context("found a tree entry without a name")?;
                    let entry_fullpath = prefix.join(entry_name);

                    let excluded = (self.exclude)
                        .iter()
                        .any(|exclude_path| exclude_path == &entry_fullpath);

                    let kept_as_is = !excluded && self.include.is_empty()
                        || (self.include)
                            .iter()
                            .any(|include_path| include_path == &entry_fullpath);

                    let rebuilt = (self.include)
                        .iter()
                        .any(|include_path| include_path.starts_with(&entry_fullpath))
                        || (kept_as_is
                            && (self.exclude)
                                .iter()
                                .any(|exclude_path| exclude_path.starts_with(&entry_fullpath)));

                    if kept_as_is {
                        builder.insert(entry_name, entry.id(), entry.filemode())?;
                    } else if rebuilt {
                        let entry_tree = self.repository.find_tree(entry.id())?;
                        let new_entry_tree = self.rebuild_tree(&entry_tree, &entry_fullpath)?;

                        if entry_tree.id() != new_entry_tree.id() {
                            changed = true;
                        }

                        builder.insert(entry_name, new_entry_tree.id(), entry.filemode())?;
                    } else {
                        self.filtered.insert(entry_fullpath);
                        changed = true;
                    }
                }

                let new_tree_id = {
                    if !changed {
                        tree.id()
                    } else {
                        builder.write()?
                    }
                };

                self.id_map.insert(tree.id(), new_tree_id);
                new_tree_id
            }
        };

        Ok(self.repository.find_tree(new_tree_id)?)
    }

    pub fn rebuild_commit(&mut self, commit: Commit<'r>) -> Result<Commit<'r>> {
        let new_commit_id = {
            if let Some(cached) = self.id_map.get(&commit.id()) {
                *cached
            } else {
                let parents: Vec<_> = commit
                    .parents()
                    .map(|p| self.rebuild_commit(p))
                    .collect::<Result<_>>()?;

                let parents_borrowed: Vec<_> = parents.iter().collect();

                let tree = self
                    .rebuild_tree(&commit.tree()?, Path::new(""))
                    .context("failed to rebuild Tree")?;

                let parents_changed = parents.iter().any(|p| self.changed(p.id()));
                let tree_changed = self.changed(commit.tree()?.id());
                let changed = parents_changed || tree_changed;

                let new_commit_id = {
                    if changed {
                        self.repository
                            .commit(
                                None,
                                &commit.author(),
                                &commit.committer(),
                                commit.message().unwrap_or(""),
                                &tree,
                                &parents_borrowed,
                            )
                            .context("failed to create Commit")?
                    } else {
                        commit.id()
                    }
                };

                self.id_map.insert(commit.id(), new_commit_id);
                new_commit_id
            }
        };

        Ok(self.repository.find_commit(new_commit_id)?)
    }
}
