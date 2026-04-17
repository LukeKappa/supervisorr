use axum::{
    routing::{get, post},
    Router,
    response::{Html, IntoResponse, Json},
};
use axum::extract::Multipart;
use tokio::fs;
use std::os::unix::fs::PermissionsExt;
use crate::daemon::state::{ProcessState, SharedState, Status, Intent};
use crate::config::ProgramConfig;
use rust_embed::Embed;
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::Arc;

#[derive(Embed)]
#[folder = "src/web/"]
struct Asset;

pub async fn start_web(state: SharedState) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/status", get({
            let state = Arc::clone(&state);
            move || api_status(state)
        }))
        .route("/api/action", post({
            let state = Arc::clone(&state);
            move |payload| api_action(state, payload)
        }))
        .route("/api/upload", post({
            let state = Arc::clone(&state);
            move |multipart| api_upload(state, multipart)
        }))
        .layer(axum::extract::DefaultBodyLimit::disable());

    let config_bind = {
        let s = state.read().await;
        s.config.supervisorr.as_ref().and_then(|sup| sup.web_bind.clone()).unwrap_or_else(|| "127.0.0.1:3000".to_string())
    };

    let addr: SocketAddr = config_bind.parse().unwrap_or_else(|_| "127.0.0.1:3000".parse().unwrap());
    println!("Web Dashboard listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn index_handler() -> impl IntoResponse {
    match Asset::get("index.html") {
        Some(file) => {
            axum::response::Response::builder()
                .header("content-type", "text/html; charset=utf-8")
                .body(axum::body::Body::from(file.data))
                .unwrap()
        }
        None => {
            axum::response::Response::builder()
                .status(404)
                .body(axum::body::Body::from("UI not found"))
                .unwrap()
        }
    }
}

#[derive(Serialize)]
struct ProcessStatusDto {
    name: String,
    status: String,
    intent: String,
}

async fn api_status(state: SharedState) -> Json<Vec<ProcessStatusDto>> {
    let mut data = Vec::new();
    let s = state.read().await;
    for (name, ps) in &s.processes {
        let status_str = match &ps.status {
            Status::Stopped => "Stopped".to_string(),
            Status::Running(pid) => format!("Running (pid {})", pid),
            Status::Exited(c) => format!("Exited (code {})", c),
            Status::Failed(e) => format!("Failed: {}", e),
        };
        let intent_str = match ps.intent {
            Intent::Run => "Run".to_string(),
            Intent::Stop => "Stop".to_string(),
        };
        data.push(ProcessStatusDto {
            name: name.clone(),
            status: status_str,
            intent: intent_str,
        });
    }
    data.sort_by(|a, b| a.name.cmp(&b.name));
    Json(data)
}

#[derive(serde::Deserialize)]
struct ActionPayload {
    action: String,
    target: String,
}

#[derive(Serialize)]
struct ActionResponse {
    success: bool,
    error: Option<String>,
}

async fn api_action(state: SharedState, axum::Json(payload): axum::Json<ActionPayload>) -> Json<ActionResponse> {
    let mut s = state.write().await;
    let target = &payload.target;
    
    if let Some(ps) = s.processes.get_mut(target) {
        if payload.action == "start" {
            ps.intent = Intent::Run;
            Json(ActionResponse { success: true, error: None })
        } else if payload.action == "stop" {
            ps.intent = Intent::Stop;
            if let Status::Running(pid) = ps.status {
                let _ = nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid as i32), nix::sys::signal::Signal::SIGTERM);
            }
            Json(ActionResponse { success: true, error: None })
        } else {
            Json(ActionResponse { success: false, error: Some("Unknown action".to_string()) })
        }
    } else {
        Json(ActionResponse { success: false, error: Some("Process not found".to_string()) })
    }
}

async fn api_upload(state: SharedState, mut multipart: Multipart) -> Json<ActionResponse> {
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("executable") {
            let mut raw_name = field.file_name().unwrap_or("uploaded_bin").to_string();
            if let Some(clean) = std::path::Path::new(&raw_name).file_name() {
                raw_name = clean.to_string_lossy().to_string();
            }
            let file_name = raw_name;
            
            let data = match field.bytes().await {
                Ok(b) => b,
                Err(e) => return Json(ActionResponse { success: false, error: Some(e.to_string()) }),
            };
            
            let current_dir = std::env::current_dir().unwrap_or_default();
            let path = current_dir.join(&file_name);
            if let Err(e) = fs::write(&path, data).await {
                return Json(ActionResponse { success: false, error: Some(format!("Failed to save: {}", e)) });
            }
            
            if let Ok(metadata) = fs::metadata(&path).await {
                let mut perms = metadata.permissions();
                perms.set_mode(0o755);
                let _ = fs::set_permissions(&path, perms).await;
            }

            let new_prog = ProgramConfig {
                command: path.to_string_lossy().to_string(),
                directory: Some(current_dir.to_string_lossy().to_string()),
                autostart: true,
                autorestart: true,
                environment: None,
                stdout_logfile: Some(current_dir.join(format!("{}.log", file_name)).to_string_lossy().to_string()),
                stderr_logfile: Some(current_dir.join(format!("{}.err", file_name)).to_string_lossy().to_string()),
            };

            {
                let mut s = state.write().await;
                s.config.program.insert(file_name.clone(), new_prog.clone());
                if let Ok(toml_str) = toml::to_string(&s.config) {
                    let _ = fs::write(&s.config_path, toml_str).await;
                }
                s.processes.insert(file_name.clone(), ProcessState {
                    intent: Intent::Run,
                    status: Status::Stopped,
                });
            }

            let state_clone = state.clone();
            tokio::spawn(async move {
                crate::daemon::supervise_program(file_name, new_prog, state_clone).await;
            });

            return Json(ActionResponse { success: true, error: None });
        }
    }
    Json(ActionResponse { success: false, error: Some("No executable found".to_string()) })
}
