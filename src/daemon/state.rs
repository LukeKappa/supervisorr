use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Intent {
    Run,
    Stop,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Stopped,
    Running(u32), // pid
    Exited(i32),  // exit code
    Failed(String), // e.g. command not found
}

#[derive(Debug, Clone)]
pub struct ProcessState {
    pub intent: Intent,
    pub status: Status,
}

pub struct AppState {
    pub processes: HashMap<String, ProcessState>,
}

impl AppState {
    pub fn new() -> Self {
        Self { processes: HashMap::new() }
    }
}

pub type SharedState = Arc<RwLock<AppState>>;
