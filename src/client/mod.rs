use tokio::net::UnixStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::daemon::ipc::{IpcRequest, IpcResponse};

async fn send_request(req: IpcRequest) -> anyhow::Result<IpcResponse> {
    // In a real app we'd get this path from args config, hardcoding for proxy demo
    let socket_path = "/tmp/supervisorr.sock"; 
    let mut stream = UnixStream::connect(socket_path).await?;
    let data = serde_json::to_vec(&req)?;
    stream.write_all(&data).await?;
    
    // Shutdown the write half so the server knows we're done sending
    // wait, json stream is single message. 
    // We can just read the response.
    
    let mut buf = vec![];
    let mut temp = vec![0; 4096];
    loop {
        let n = stream.read(&mut temp).await?;
        if n == 0 { break; }
        buf.extend_from_slice(&temp[..n]);
    }
    
    let res: IpcResponse = serde_json::from_slice(&buf)?;
    Ok(res)
}

pub async fn status() -> anyhow::Result<()> {
    match send_request(IpcRequest::Status).await? {
        IpcResponse::StatusData(data) => {
            if data.is_empty() {
                println!("No processes configured.");
            }
            for (k, v) in data {
                println!("{:<20} {}", k, v);
            }
        }
        IpcResponse::Error(e) => println!("Error: {}", e),
        _ => println!("Unexpected response"),
    }
    Ok(())
}

pub async fn start(target: &str) -> anyhow::Result<()> {
    match send_request(IpcRequest::Start { target: target.to_string() }).await? {
        IpcResponse::Ok => println!("Started {}", target),
        IpcResponse::Error(e) => println!("Error: {}", e),
        _ => println!("Unexpected response"),
    }
    Ok(())
}

pub async fn stop(target: &str) -> anyhow::Result<()> {
    match send_request(IpcRequest::Stop { target: target.to_string() }).await? {
        IpcResponse::Ok => println!("Stopped {}", target),
        IpcResponse::Error(e) => println!("Error: {}", e),
        _ => println!("Unexpected response"),
    }
    Ok(())
}
