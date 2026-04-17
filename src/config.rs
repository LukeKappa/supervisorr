use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supervisorr: Option<SupervisorrConfig>,
    #[serde(default)]
    pub program: HashMap<String, ProgramConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SupervisorrConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub socket_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_bind: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProgramConfig {
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directory: Option<String>,
    #[serde(default = "default_true")]
    pub autostart: bool,
    #[serde(default = "default_true")]
    pub autorestart: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout_logfile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr_logfile: Option<String>,
}

fn default_true() -> bool { true }
