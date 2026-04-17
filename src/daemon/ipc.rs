use crate::daemon::state::{SharedState, Intent, Status};
use tokio::net::{UnixListener, UnixStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use serde::{Deserialize, Serialize};
use std::fs;
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use std::sync::Arc;
use std::os::unix::fs::PermissionsExt;

#[derive(Serialize, Deserialize, Debug)]
pub enum IpcRequest {
    Status,
    Start { target: String },
    Stop { target: String },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum IpcResponse {
    StatusData(std::collections::HashMap<String, String>),
    Ok,
    Error(String),
}

pub async fn setup_ipc(socket_path: &str, state: SharedState) -> anyhow::Result<()> {
    if fs::metadata(socket_path).is_ok() {
        fs::remove_file(socket_path)?;
    }

    let listener = UnixListener::bind(socket_path)?;
    if let Ok(metadata) = fs::metadata(socket_path) {
        let mut perms = metadata.permissions();
        perms.set_mode(0o600);
        let _ = fs::set_permissions(socket_path, perms);
    }
    
    println!("IPC Server listening on {}", socket_path);

    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let state_clone = Arc::clone(&state);
                tokio::spawn(async move {
                    if let Err(e) = handle_client(stream, state_clone).await {
                        eprintln!("Error handling client: {}", e);
                    }
                });
            }
            Err(e) => {
                eprintln!("Failed to accept incoming IPC connection: {}", e);
            }
        }
    }
}

async fn handle_client(mut stream: UnixStream, state: SharedState) -> anyhow::Result<()> {
    let mut buf = vec![0; 4096];
    let n = stream.read(&mut buf).await?;
    if n == 0 { return Ok(()); }

    let req: IpcRequest = match serde_json::from_slice(&buf[..n]) {
        Ok(r) => r,
        Err(e) => {
            let res = IpcResponse::Error(format!("Invalid request: {}", e));
            let data = serde_json::to_vec(&res)?;
            stream.write_all(&data).await?;
            return Ok(());
        }
    };

    let response = process_request(req, state).await;
    let data = serde_json::to_vec(&response)?;
    stream.write_all(&data).await?;
    
    Ok(())
}

async fn process_request(req: IpcRequest, state: SharedState) -> IpcResponse {
    match req {
        IpcRequest::Status => {
            let mut data = std::collections::HashMap::new();
            let s = state.read().await;
            for (name, ps) in &s.processes {
                let status_str = match &ps.status {
                    Status::Stopped => "STOPPED".to_string(),
                    Status::Running(pid) => format!("RUNNING (pid {})", pid),
                    Status::Exited(c) => format!("EXITED (code {})", c),
                    Status::Failed(e) => format!("FAILED ({})", e),
                };
                let intent_str = match ps.intent {
                    Intent::Run => "intended: RUN",
                    Intent::Stop => "intended: STOP",
                };
                data.insert(name.clone(), format!("{} [{}]", status_str, intent_str));
            }
            IpcResponse::StatusData(data)
        }
        IpcRequest::Start { target } => {
            let mut s = state.write().await;
            if let Some(ps) = s.processes.get_mut(&target) {
                ps.intent = Intent::Run;
                IpcResponse::Ok
            } else {
                IpcResponse::Error("Process not found".to_string())
            }
        }
        IpcRequest::Stop { target } => {
            let mut s = state.write().await;
            if let Some(ps) = s.processes.get_mut(&target) {
                ps.intent = Intent::Stop;
                if let Status::Running(pid) = ps.status {
                    // Send SIGTERM
                    let _ = kill(Pid::from_raw(pid as i32), Signal::SIGTERM);
                }
                IpcResponse::Ok
            } else {
                IpcResponse::Error("Process not found".to_string())
            }
        }
    }
}
