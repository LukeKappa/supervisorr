use axum::{
    routing::{get, post},
    Router,
    response::{Html, IntoResponse, Json},
};
use rust_embed::Embed;
use crate::daemon::state::{SharedState, Status, Intent};
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
        }));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
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
