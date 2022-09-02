mod config;

use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use git2::{BranchType, Commit, Cred, Oid, PushOptions, RemoteCallbacks, Repository, Tree};

use config::Config;

const DEFAULT_CONFIG_PATH: &str = ".git-publish/config.yml";
const WORKING_BRANCH_NAME: &str = "git-publish_working_branch";

struct Rebuilder<'c, 'r> {
    repository: &'r Repository,
    include: &'c [PathBuf],

    /// Map of required objects IDs (old_id -> new_id)
    id_map: HashMap<Oid, Oid>,

    /// Keep track of paths that have been filtered-out
    filtered: HashSet<PathBuf>,
}

impl<'c, 'r> Rebuilder<'c, 'r> {
    fn new(repository: &'r Repository, include: &'c [PathBuf]) -> Self {
        Self {
            repository,
            id_map: HashMap::new(),
            include,
            filtered: HashSet::new(),
        }
    }

    fn debug_filtered(&self) {
        println!("Filtered {} paths:", self.filtered.len());
        let mut sorted_vec: Vec<_> = self.filtered.iter().collect();
        sorted_vec.sort_unstable();

        for path in sorted_vec {
            println!("  - {}", path.display());
        }
    }

    fn changed(&self, id: Oid) -> bool {
        self.id_map.get(&id).unwrap_or(&id) != &id
    }

    fn rebuild_tree(&mut self, tree: &Tree<'r>, prefix: &Path) -> Result<Tree<'r>> {
        let new_tree_id = {
            if let Some(cached) = self.id_map.get(&tree.id()) {
                *cached
            } else {
                let mut changed = false;
                let mut builder = self.repository.treebuilder(Some(&tree))?;

                for entry in tree {
                    let entry_name = entry.name().context("found a tree entry without a name")?;
                    let entry_fullpath = prefix.join(entry_name);

                    let kept_as_is = (self.include)
                        .iter()
                        .any(|include_path| include_path == &entry_fullpath);

                    let rebuilt = (self.include)
                        .iter()
                        .any(|include_path| include_path.starts_with(&entry_fullpath));

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

    fn rebuild_commit(&mut self, commit: Commit<'r>) -> Result<Commit<'r>> {
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
                        let new_id = self
                            .repository
                            .commit(
                                None,
                                &commit.author(),
                                &commit.committer(),
                                commit.message().unwrap_or(""),
                                &tree,
                                &parents_borrowed,
                            )
                            .context("failed to create Commit")?;

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

fn read_config(repository: &Repository) -> Result<Config> {
    let config_path = repository
        .path()
        .parent() // Repository::path() points to the .git directory
        .unwrap()
        .join(DEFAULT_CONFIG_PATH);

    println!("Reading config from {}", config_path.display());
    let mut config_data = Vec::new();

    File::open(config_path)
        .context("failed to open config file")?
        .read_to_end(&mut config_data)
        .context("failed to read config file")?;

    serde_yaml::from_slice(&config_data).context("invalid config format")
}

fn main() -> Result<()> {
    let rep = Repository::open_from_env()?;
    let config = read_config(&rep).context("could not load config")?;

    for remote_config in &config.remotes {
        let commit = {
            let master = rep.find_branch("master", BranchType::Local)?;
            master.get().peel_to_commit()?
        };

        let mut rebuilder = Rebuilder::new(&rep, &remote_config.include);
        let new_commit = rebuilder.rebuild_commit(commit)?;
        rep.branch(WORKING_BRANCH_NAME, &new_commit, true)?;
        rebuilder.debug_filtered();

        let push_cb = || {
            let mut push_cb = RemoteCallbacks::new();

            push_cb
                .credentials(|_url, username_from_url, _allowed_types| {
                    Cred::ssh_key(
                        username_from_url.unwrap(),
                        None,
                        std::path::Path::new(&format!(
                            "{}/.ssh/id_ed25519",
                            std::env::var("HOME").unwrap()
                        )),
                        None,
                    )
                })
                .transfer_progress(|p| {
                    println!("{}/{}", p.indexed_objects(), p.total_objects());
                    println!("{}/{}", p.indexed_deltas(), p.total_deltas());
                    true
                })
                .push_update_reference(|reference, status| {
                    if let Some(msg) = status {
                        println!(r"/!\ failed to push {reference}: {msg}");
                    } else {
                        println!("Successfully pushed {reference}");
                    }

                    Ok(())
                });

            push_cb
        };

        let mut push_opt = PushOptions::new();
        push_opt.remote_callbacks(push_cb());

        let mut remote = rep.remote_anonymous(&remote_config.url)?;

        remote.push(
            &[format!(
                "refs/heads/{WORKING_BRANCH_NAME}:refs/heads/master-rebuilt"
            )],
            Some(&mut push_opt),
        )?;

        remote.update_tips(
            Some(&mut push_cb()),
            false,
            git2::AutotagOption::Auto,
            Some("test"),
        )?;
    }

    Ok(())
}
