use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use anyhow::{Context, Result};
use git2::Repository;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub remotes: Vec<RemoteConfig>,
}

#[derive(Deserialize)]
pub struct RemoteConfig {
    pub url: String,
    pub include: Vec<PathBuf>,
}

const DEFAULT_CONFIG_PATH: &str = ".git-publish/config.yml";

pub fn read_config(repository: &Repository) -> Result<Config> {
    let config_path = repository
        .path()
        .parent() // Repository::path() points to the .git directory
        .unwrap()
        .join(DEFAULT_CONFIG_PATH);

    let mut config_data = Vec::new();

    File::open(config_path)
        .context("failed to open config file")?
        .read_to_end(&mut config_data)
        .context("failed to read config file")?;

    serde_yaml::from_slice(&config_data).context("invalid config format")
}
