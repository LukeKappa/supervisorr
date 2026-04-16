pub mod ipc;
pub mod web;
pub mod state;

use crate::config::{Config, ProgramConfig};
use state::{AppState, SharedState, ProcessState, Intent, Status};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::process::Command;
use std::process::Stdio;
use std::fs::OpenOptions;

pub async fn run(config_path: &str) -> anyhow::Result<()> {
    if !std::path::Path::new(config_path).exists() {
        eprintln!("Configuration file not found at: {}", config_path);
        eprintln!("Run `supervisorr init` to generate a default configuration file in your current directory.");
        std::process::exit(1);
    }
    println!("Starting supervisorr daemon using config: {}", config_path);
    let config_content = std::fs::read_to_string(config_path).unwrap_or_else(|_| "".to_string());
    
    let config: Config = if config_content.is_empty() {
        Config { supervisorr: None, program: std::collections::HashMap::new() }
    } else {
        toml::from_str(&config_content)?
    };

    let state = Arc::new(RwLock::new(AppState::new()));

    for (name, prog_config) in config.program.into_iter() {
        let intent = if prog_config.autostart { Intent::Run } else { Intent::Stop };
        state.write().await.processes.insert(name.clone(), ProcessState {
            intent,
            status: Status::Stopped,
        });

        let state_clone = Arc::clone(&state);
        tokio::spawn(async move {
            supervise_program(name, prog_config, state_clone).await;
        });
    }

    let socket_path = config.supervisorr.and_then(|s| s.socket_file).unwrap_or_else(|| "/tmp/supervisorr.sock".to_string());
    
    let state_clone = Arc::clone(&state);
    tokio::spawn(async move {
        if let Err(e) = ipc::setup_ipc(&socket_path, state_clone).await {
            eprintln!("IPC server failed: {}", e);
        }
    });

    let state_clone_web = Arc::clone(&state);
    tokio::spawn(async move {
        if let Err(e) = web::start_web(state_clone_web).await {
            eprintln!("Web server failed: {}", e);
        }
    });

    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("Received SIGINT. Shutting down.");
        }
        _ = sigterm.recv() => {
            println!("Received SIGTERM. Shutting down.");
        }
    }
    
    // Cleanup socket file on exit
    let _ = std::fs::remove_file("/tmp/supervisorr.sock");
    
    Ok(())
}

async fn supervise_program(name: String, config: ProgramConfig, state: SharedState) {
    loop {
        let intent = {
            let s = state.read().await;
            s.processes.get(&name).map(|ps| ps.intent).unwrap_or(Intent::Stop)
        };

        if intent == Intent::Stop {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            continue;
        }

        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(&config.command);
        if let Some(dir) = &config.directory {
            cmd.current_dir(dir);
        }
        if let Some(envs) = &config.environment {
            cmd.envs(envs);
        }
        
        if let Some(out_log) = &config.stdout_logfile {
            if let Ok(file) = OpenOptions::new().create(true).append(true).open(out_log) {
                cmd.stdout(Stdio::from(file));
            } else {
                cmd.stdout(Stdio::null());
            }
        } else {
            cmd.stdout(Stdio::null());
        }

        if let Some(err_log) = &config.stderr_logfile {
            if let Ok(file) = OpenOptions::new().create(true).append(true).open(err_log) {
                cmd.stderr(Stdio::from(file));
            } else {
                cmd.stderr(Stdio::null());
            }
        } else {
            cmd.stderr(Stdio::null());
        }

        match cmd.spawn() {
            Ok(mut child) => {
                let pid = child.id().unwrap_or(0);
                {
                    let mut s = state.write().await;
                    if let Some(ps) = s.processes.get_mut(&name) {
                        ps.status = Status::Running(pid);
                    }
                }

                let status = child.wait().await;
                
                let exit_code = match status {
                    Ok(exit_status) => exit_status.code().unwrap_or(-1),
                    Err(_) => -1,
                };
                
                {
                    let mut s = state.write().await;
                    if let Some(ps) = s.processes.get_mut(&name) {
                        ps.status = Status::Exited(exit_code);
                    }
                }
            }
            Err(e) => {
                let mut s = state.write().await;
                if let Some(ps) = s.processes.get_mut(&name) {
                    ps.status = Status::Failed(e.to_string());
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        }

        let intent = {
            let s = state.read().await;
            s.processes.get(&name).map(|ps| ps.intent).unwrap_or(Intent::Stop)
        };
        
        if intent == Intent::Stop || !config.autorestart {
            while {
                let s = state.read().await;
                s.processes.get(&name).map(|ps| ps.intent).unwrap_or(Intent::Stop) != Intent::Run
            } {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            }
        } else {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
    }
}
