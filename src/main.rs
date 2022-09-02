mod config;
mod git_utils;

use anyhow::{Context, Result};
use std::fs::File;
use std::io::Read;

use git2::{BranchType, PushOptions, Repository};

use config::Config;
use git_utils::build_remote_callbacks;
use git_utils::rebuilder::Rebuilder;

const DEFAULT_CONFIG_PATH: &str = ".git-publish/config.yml";
const WORKING_BRANCH_NAME: &str = "git-publish_working_branch";

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

        rebuilder.debug_changes();
        rebuilder.debug_filtered();

        // Push
        let mut push_opt = PushOptions::new();
        push_opt.remote_callbacks(build_remote_callbacks()?);

        let mut remote = rep.remote_anonymous(&remote_config.url)?;

        remote.push(
            &[format!(
                "+refs/heads/{WORKING_BRANCH_NAME}:refs/heads/master-rebuilt"
            )],
            Some(&mut push_opt),
        )?;
    }

    Ok(())
}
