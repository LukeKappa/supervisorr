use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub supervisorr: Option<SupervisorrConfig>,
    #[serde(default)]
    pub program: HashMap<String, ProgramConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SupervisorrConfig {
    pub socket_file: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ProgramConfig {
    pub command: String,
    pub directory: Option<String>,
    #[serde(default = "default_true")]
    pub autostart: bool,
    #[serde(default = "default_true")]
    pub autorestart: bool,
    pub environment: Option<HashMap<String, String>>,
    pub stdout_logfile: Option<String>,
    pub stderr_logfile: Option<String>,
}

fn default_true() -> bool { true }
