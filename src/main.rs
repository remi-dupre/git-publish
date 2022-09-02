use std::collections::HashMap;

use git2::{BranchType, Commit, Cred, Oid, PushOptions, RemoteCallbacks, Repository, Tree};

pub type Result<T> = std::result::Result<T, git2::Error>;

struct Rebuilder<'r> {
    repository: &'r Repository,
    id_map: HashMap<Oid, Oid>,
}

impl<'r> Rebuilder<'r> {
    pub fn new(repository: &'r Repository) -> Self {
        Self {
            repository,
            id_map: HashMap::new(),
        }
    }

    pub fn changed(&self, id: Oid) -> bool {
        self.id_map.get(&id).unwrap_or(&id) != &id
    }

    pub fn rebuild_tree(&mut self, tree: Tree<'r>) -> Result<Tree<'r>> {
        let new_tree_id = {
            if let Some(cached) = self.id_map.get(&tree.id()) {
                *cached
            } else {
                let mut changed = false;
                let mut builder = self.repository.treebuilder(Some(&tree))?;

                builder.filter(|e| {
                    let removed = matches!(e.name(), Some("ci" | ".gitlab-ci.yml"));
                    changed |= removed;
                    !removed
                })?;

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
                let parents_changed = parents.iter().any(|p| self.changed(p.id()));
                let tree = self.rebuild_tree(commit.tree()?)?;
                let changed = parents_changed || self.changed(commit.tree()?.id());

                let new_commit_id = {
                    if changed {
                        let new_id = self.repository.commit(
                            None,
                            &commit.author(),
                            &commit.committer(),
                            commit.message().unwrap_or(""),
                            &tree,
                            &parents_borrowed,
                        )?;
                        eprintln!("{} -> {}", commit.id(), new_id);
                        new_id
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

fn main() -> Result<()> {
    let rep = Repository::open_from_env()?;

    let commit = {
        let master = rep.find_branch("master", BranchType::Local)?;
        master.get().peel_to_commit()?
    };

    let mut rebuilder = Rebuilder::new(&rep);
    let new_commit = rebuilder.rebuild_commit(commit)?;
    rep.branch("master-rebuilt", &new_commit, true)?;

    let push_cb = || {
        let mut push_cb = RemoteCallbacks::new();

        push_cb.credentials(|_url, username_from_url, _allowed_types| {
            println!("{} {:?} {:?}", _url, username_from_url, _allowed_types);
            Cred::ssh_key(
                username_from_url.unwrap(),
                None,
                std::path::Path::new(&format!(
                    "{}/.ssh/id_ed25519",
                    std::env::var("HOME").unwrap()
                )),
                None,
            )
        });

        push_cb.transfer_progress(|p| {
            eprintln!("{}/{}", p.indexed_objects(), p.total_objects());
            eprintln!("{}/{}", p.indexed_deltas(), p.total_deltas());
            true
        });

        push_cb.push_update_reference(|reference, status| {
            if let Some(msg) = status {
                eprintln!(r"/!\ failed to push {reference}: {msg}");
            }

            Ok(())
        });

        push_cb
    };

    let mut push_opt = PushOptions::new();
    push_opt.remote_callbacks(push_cb());

    let mut remote = rep.remote_anonymous("git@github.com:Qwant/fafnir.git")?;

    remote.push(
        &["refs/heads/master-rebuilt:refs/heads/master-rebuilt"],
        Some(&mut push_opt),
    )?;

    remote.update_tips(
        Some(&mut push_cb()),
        false,
        git2::AutotagOption::Auto,
        Some("test"),
    )?;

    Ok(())
}
