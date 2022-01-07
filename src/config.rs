use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use color_eyre::eyre::Context;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Config {
    /// The projects defined in this workspace
    pub project: BTreeMap<String, ProjectConfig>,
    /// General configuration
    pub general: GeneralConfig,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct GeneralConfig {
    /// The path of the `src` dir that contains all projects
    ///
    /// relative to the directory of the config file
    ///
    /// defaults to `src`
    #[serde(default = "source_dir")]
    pub src: PathBuf,
}

fn config_txt() -> PathBuf {
    PathBuf::from("config.txt")
}

fn source_dir() -> PathBuf {
    PathBuf::from("src")
}

fn cache_dir() -> PathBuf {
    PathBuf::from("cache")
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
/// A single project
pub struct ProjectConfig {
    /// Root directory of the project
    ///
    /// relative to `general.src` (default to `<name>`)
    pub dir: Option<String>,
    /// Name of the config file
    ///
    /// relative to `project.<name>.dir`
    ///
    /// defaults to `config.txt`
    #[serde(default = "config_txt")]
    pub config: PathBuf,
    /// Path to the cache dir
    ///
    /// relative to the directory of the config file
    #[serde(default = "cache_dir")]
    pub cache: PathBuf,
    /// Name of the cache subdirectory
    ///
    /// relative to `project.<name>.cache`
    ///
    /// defaults to `<name>`
    pub key: Option<String>,

    /// Name of the generated PKI file
    ///
    /// relative to `{cache}/{key}`
    #[serde(default = "pk_index")]
    pub pki: PathBuf,

    /// Name of the generated manifest file
    ///
    /// relative to `{cache}/{key}`
    #[serde(default = "default_manifest")]
    pub manifest: PathBuf,
}

fn pk_index() -> PathBuf {
    PathBuf::from("primary")
}

fn default_manifest() -> PathBuf {
    PathBuf::from("trunk")
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> color_eyre::Result<Self> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)
            .wrap_err_with(|| format!("Failed to load {}", path.display()))?;
        let cfg: Self = toml::from_str(&text)
            .wrap_err_with(|| format!("Failed to parse config {}", path.display()))?;
        Ok(cfg)
    }
}
