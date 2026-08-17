# Task Manager

A lightweight, high-performance task queue manager refactored from FastAPI into Rust. Powered by Axum, Tokio, and SQLite.

## Features

- **Blazing Fast**: Asynchronous request processing backed by Axum and Tokio.
- **Persistent Storage**: Embedded SQLite database initialized automatically via SQLx.
- **Secure by Default**: Native Token authentication protection (`X-API-Token`) on all endpoints.
- **Service Ready**: Native systemd integration and ready-to-deploy Debian packaging.

---

## 🚀 Quick Start (Debian / Ubuntu Production Deployment)

### 1. Installation
Install the pre-compiled `.deb` package directly onto your host system:
```bash
sudo apt install ./task-manager_0.1.0-1_amd64.deb
```
*Note: During installation, an isolated environment file `/etc/task-manager.env` is automatically provisioned with a secure, randomly generated 32-byte hex token.*

### 2. Service Management
The package automatically registers and starts a background `systemd` daemon:
```bash
# Check runtime status
systemctl status task-manager

# Stream application logs
journalctl -u task-manager -f

# Restart or stop the service
systemctl restart task-manager
systemctl stop task-manager
```

## Running Binaries

If you downloaded the standalone raw executable files (from the GitHub Releases page) instead of using the `.deb` installer, you must provide the configuration environment variables manually before executing the file.

### Linux / macOS
Open your terminal, navigate to the folder containing your binary, and execute the following sequence:

```bash
# 1. (Optional) Grant executable permissions if needed
chmod +x task-manager-linux-amd64

# 2. Export required runtime environment variables
export HOST="127.0.0.1"
export PORT="33601"
export DB_FILE="./tasks.db"
export SECRET_TOKEN="YourSuperSecureLongTokenHere123!"

# 3. Launch the application binary
./task-manager-linux-amd64
```

### Windows
Launch your PowerShell terminal in your executable's path and configure your session environment variables:

```powershell
# 1. Declare session variables 
\$env:HOST="127.0.0.1"
\$env:PORT="33601"
\$env:DB_FILE=".\tasks.db"
\$env:SECRET_TOKEN="YourSuperSecureLongTokenHere123!"

# 2. Execute the app
.\task-manager-windows-amd64.exe
```

*Note: When running standalone binaries, the SQLite database (`DB_FILE`) will be automatically initialized relative to your specified string path upon the first incoming request.*

---

## API Reference

The service binds to port **`33601`** by default. Retrieve your token from `/etc/task-manager.env` and replace `$YOUR_TOKEN` below.

### Import Tasks
Queue new task IDs into the database (`PENDING` state). Duplicate IDs are safely ignored.
```bash
curl -X POST http://127.0.0.1:33601/tasks/import \
     -H 'Content-Type: application/json' \
     -H 'X-API-Token: \$YOUR_TOKEN' \
     -d '{"task_ids": ["task_001", "task_002"]}'
```

### Pop a Task
Atomic operation fetching the next available `PENDING` task and transitioning its state to `RUNNING`.
```bash
curl -X GET http://127.0.0.1:8000/tasks/pop \
     -H 'X-API-Token: \$YOUR_TOKEN'
```

### Complete Task
Mark a `RUNNING` task as successfully finalized (`SUCCESS` state).
```bash
curl -X POST http://127.0.0.1:8000/tasks/task_001/complete \
     -H 'X-API-Token: \$YOUR_TOKEN'
```

### Get Stats
Retrieve real-time metrics showing total counts broken down by status.
```bash
curl -X GET http://127.0.0.1:8000/tasks/stats \
     -H 'X-API-Token: \$YOUR_TOKEN'
```

### Reset Tasks
* **Reset all stalled `RUNNING` tasks** back to `PENDING`:
  ```bash
  curl -X POST http://127.0.0.1:8000/tasks/reset \
       -H 'X-API-Token: \$YOUR_TOKEN'
  ```
* **Reset a specific task** by ID:
  ```bash
  curl -X POST http://127.0.0.1:8000/tasks/reset?task_id=task_001 \
       -H 'X-API-Token: \$YOUR_TOKEN'
  ```

### Clear Tasks
Flush or remove target tasks out of the manager index.
```bash
curl -X POST http://127.0.0.1:8000/tasks/clear \
     -H 'X-API-Token: \$YOUR_TOKEN'
```

---

## Configuration

The application evaluates the following variables inside `/etc/task-manager.env`:

| Variable | Default Value | Description |
| :--- | :--- | :--- |
| `HOST` | `0.0.0.0` | Target binding IP interface address |
| `PORT` | `33601` | Active web server incoming network port |
| `DB_FILE` | `/var/lib/task-manager/tasks.db` | Path targeting local persistent SQLite instance |
| `SECRET_TOKEN` | *(Generated on install)* | Required bearer value matched against incoming headers |

---

## Local Compilation

If building from raw source without using the Debian installer framework:

```bash
# 1. Compile release optimized binary
cargo build --release

# 2. Assign environment declarations manually
export SECRET_TOKEN="YourSuperSecureLongTokenHere123!"
export PORT="33601"

# 3. Run the executable locally
./target/release/task-manager
```

## Automated Releases

Multi-platform binary builds are fully automated. Simply create and push a standard version tag:
```bash
git tag v0.1.0
git push origin v0.1.0
```


