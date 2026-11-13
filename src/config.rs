use crate::util;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const DEFAULT_PORT: u16 = 8080;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub port: u16,
    pub heap_mb: u32,
    pub metaspace_mb: u32,
    pub executors: u32,
    pub jenkins_version: String,
    /// URL pública explícita. Vacía = calcular la IP local automáticamente.
    #[serde(default)]
    pub url: String,
    pub root: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: DEFAULT_PORT,
            heap_mb: 1200,
            metaspace_mb: 512,
            executors: 2,
            jenkins_version: String::new(),
            url: String::new(),
            root: util::home().join("jenkins"),
        }
    }
}

pub fn config_path() -> PathBuf {
    util::home().join(".config/termux-cicd/config.toml")
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("no se pudo leer {}", path.display()))?;
        let mut cfg: Self = toml::from_str(&raw)
            .with_context(|| format!("configuración inválida en {}", path.display()))?;
        cfg.root = util::expand_home(&cfg.root);
        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let raw = toml::to_string_pretty(self)?;
        fs::write(&path, raw)
            .with_context(|| format!("no se pudo guardar {}", path.display()))
    }

    pub fn paths(&self) -> Paths {
        Paths::new(self.root.clone())
    }
}

#[derive(Debug, Clone)]
pub struct Paths {
    pub root: PathBuf,
}

impl Paths {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn home(&self) -> PathBuf {
        self.root.join("home")
    }

    pub fn war(&self) -> PathBuf {
        self.root.join("jenkins.war")
    }

    pub fn plugins(&self) -> PathBuf {
        self.home().join("plugins")
    }

    pub fn init_dir(&self) -> PathBuf {
        self.home().join("init.groovy.d")
    }

    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }

    pub fn log_file(&self) -> PathBuf {
        self.logs().join("current")
    }

    pub fn password(&self) -> PathBuf {
        self.home().join("admin-password.txt")
    }

    pub fn url_file(&self) -> PathBuf {
        self.root.join("url.txt")
    }

    pub fn version_file(&self) -> PathBuf {
        self.root.join(".version")
    }

    pub fn jpm(&self) -> PathBuf {
        self.root.join("jenkins-plugin-manager.jar")
    }

    pub fn cli_jar(&self) -> PathBuf {
        self.root.join("jenkins-cli.jar")
    }

    pub fn plugins_txt(&self) -> PathBuf {
        self.root.join("plugins.txt")
    }

    pub fn init_script(&self) -> PathBuf {
        self.init_dir().join("00-security.groovy")
    }
}
