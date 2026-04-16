# Supervisorr

A zero-dependency, ultra-low-memory process supervisor in Rust, perfect for edge devices, old ARM hardware, or minimalist container setups. `supervisorr` is designed to be a drop-in single-binary replacement for legacy Python-based `supervisord`, giving you the same control without the bloat.

## Features

- **Micro Footprint**: Statically compiled asynchronous Rust daemon using Tokio. Zero system library dependencies required.
- **Embedded Web Dashboard**: Fully featured, interactive HTML/JS dashboard embedded directly in the binary using `axum` and `rust-embed`. Manage your cluster securely from the browser on port `3000`.
- **Integrated Logging**: Native `stdout` and `stderr` routing explicitly to target log files configured per-process.
- **IPC UNIX API**: All commands are safely executed over a local Unix domain socket (`/tmp/supervisorr.sock`) using JSON. Connect your own tools identically to our CLI!
- **Graceful Takedowns**: Natively listens to `SIGINT` and `SIGTERM` to safely terminate workers and clean up OS socket bindings.

## Configuration

Place your config under `/etc/supervisorr/supervisorr.toml` or load it specifically via `-c`.

```toml
[program.my_app]
command = "node index.js"
directory = "/var/www/my_app"
autostart = true
autorestart = true
stdout_logfile = "/var/log/my_app.log"
stderr_logfile = "/var/log/my_app.err"

[program.my_app.environment]
PORT = "8080"
NODE_ENV = "production"
```

## Usage

Start the Daemon:
```bash
./supervisorr daemon -c /path/to/supervisorr.toml
```

Manage Processes via Client CLI:
```bash
# Check the status of all managed applications
./supervisorr status

# Start or stop a target process
./supervisorr start my_app
./supervisorr stop my_app
```

## API Endpoint
The web dashboard listens by default on `http://0.0.0.0:3000`.  
Interact directly programmatically:
```bash
curl -X POST http://127.0.0.1:3000/api/action \
-H "Content-Type: application/json" \
-d '{"action":"start","target":"my_app"}'
```
