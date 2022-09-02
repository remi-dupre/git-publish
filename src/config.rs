use std::path::PathBuf;

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
