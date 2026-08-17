use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::{SqlitePoolOptions, SqliteConnectOptions}, Pool, Sqlite};
use std::str::FromStr;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;

// global state
struct AppState {
    db: Pool<Sqlite>,
    secret_token: String,
}

// error
#[derive(Serialize)]
struct ApiError {
    detail: String,
}

// request
#[derive(Deserialize)]
struct ImportTasksRequest {
    task_ids: Vec<String>,
}

#[derive(Deserialize)]
struct ResetQuery {
    task_id: Option<String>,
}

#[tokio::main]
async fn main() {
    // env
    let db_file = env::var("DB_FILE").unwrap_or_else(|_| "tasks.db".to_string());
    //let secret_token = env::var("SECRET_TOKEN")
    //    .expect("set SECRET_TOKEN = xxxxxxxxxx");
    let secret_token = match env::var("SECRET_TOKEN") {
        Ok(val) => val,
        Err(_) => {
            eprintln!("Error: SECRET_TOKEN environment variable is not set!");
            std::process::exit(1);
        }
    };
    let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port_str = env::var("PORT").unwrap_or_else(|_| "8000".to_string());
    let port: u16 = port_str
        .parse()
        .expect("set PORT = 8000");

    // init
    let database_url = format!("sqlite:{}", db_file);
    
    // auto create if not exist
    let connect_options = SqliteConnectOptions::from_str(&database_url)
        .expect("Failed to parse database URL")
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(connect_options)
        .await
        .expect("connect database error");

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS tasks (
            id TEXT PRIMARY KEY, 
            status TEXT DEFAULT 'PENDING'
         )"
    )
    .execute(&pool)
    .await
    .expect("init database error");

    let shared_state = Arc::new(AppState {
        db: pool,
        secret_token,
    });

    // router
    let app = Router::new()
        .route("/tasks/import", post(import_tasks))
        .route("/tasks/pop", get(pop_task))
        .route("/tasks/:task_id/complete", post(complete_task))
        .route("/tasks/stats", get(get_stats))
        .route("/tasks/reset", post(reset_tasks))
        .route("/tasks/clear", post(clear_tasks))
        .with_state(shared_state);

    // serve
    let bind_address = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&bind_address)
        .await
        .unwrap_or_else(|_| panic!("bind error: {}", bind_address));
    println!("listening port: http://{}", bind_address);
    axum::serve(listener, app).await.unwrap();
}

// check token
fn verify_token(headers: &HeaderMap, secret_token: &str) -> Result<(), (StatusCode, Json<ApiError>)> {
    if let Some(token) = headers.get("X-API-Token") {
        if token.to_str().unwrap_or("") == secret_token {
            return Ok(());
        }
    }
    Err((
        StatusCode::UNAUTHORIZED,
        Json(ApiError { detail: "Unauthorized".to_string() }),
    ))
}

// 1. import tasks
async fn import_tasks(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<ImportTasksRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    verify_token(&headers, &state.secret_token)?;

    let mut tx = state.db.begin().await.map_err(|_| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { detail: "DB Error".into() }))
    })?;

    for tid in &payload.task_ids {
        sqlx::query("INSERT OR IGNORE INTO tasks (id) VALUES (?)")
            .bind(tid)
            .execute(&mut *tx)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { detail: "DB Error".into() })))?;
    }
    
    tx.commit().await.map_err(|_| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { detail: "DB Error".into() }))
    })?;

    Ok(Json(serde_json::json!({
        "status": "success",
        "imported": payload.task_ids.len()
    })))
}

// 2. get task
async fn pop_task(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    verify_token(&headers, &state.secret_token)?;

    let mut tx = state.db.begin().await.map_err(|_| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { detail: "DB Transaction Error".into() }))
    })?;

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM tasks WHERE status = 'PENDING' LIMIT 1"
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { detail: "DB Query Error".into() })))?;

    if let Some((task_id,)) = row {
        sqlx::query("UPDATE tasks SET status = 'RUNNING' WHERE id = ?")
            .bind(&task_id)
            .execute(&mut *tx)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { detail: "DB Update Error".into() })))?;

        tx.commit().await.map_err(|_| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { detail: "DB Commit Error".into() }))
        })?;

        Ok(Json(serde_json::json!({ "task_id": task_id })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(ApiError { detail: "No pending tasks available".to_string() }),
        ))
    }
}

// 3. complete task
async fn complete_task(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(task_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    verify_token(&headers, &state.secret_token)?;

    let result = sqlx::query("UPDATE tasks SET status = 'SUCCESS' WHERE id = ? AND status = 'RUNNING'")
        .bind(&task_id)
        .execute(&state.db)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { detail: "DB Error".into() })))?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError { detail: "Task not found or not in RUNNING status".to_string() }),
        ));
    }

    Ok(Json(serde_json::json!({ "status": "success", "task_id": task_id })))
}

// 4. view stats
async fn get_stats(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<HashMap<String, i64>>, (StatusCode, Json<ApiError>)> {
    verify_token(&headers, &state.secret_token)?;

    let rows: Vec<(String, i64)> = sqlx::query_as("SELECT status, COUNT(*) FROM tasks GROUP BY status")
        .fetch_all(&state.db)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { detail: "DB Error".into() })))?;

    let stats: HashMap<String, i64> = rows.into_iter().collect();
    Ok(Json(stats))
}

// 5. reset tasks (query ：/tasks/reset?task_id=xxx)
async fn reset_tasks(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<ResetQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    verify_token(&headers, &state.secret_token)?;

    let msg = if let Some(task_id) = query.task_id {
        let result = sqlx::query("UPDATE tasks SET status = 'PENDING' WHERE id = ?")
            .bind(&task_id)
            .execute(&state.db)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { detail: "DB Error".into() })))?;

        if result.rows_affected() == 0 {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ApiError { detail: format!("Task {} not found", task_id) }),
            ));
        }
        format!("Task {} has been reset to PENDING", task_id)
    } else {
        let result = sqlx::query("UPDATE tasks SET status = 'PENDING' WHERE status = 'RUNNING'")
            .execute(&state.db)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { detail: "DB Error".into() })))?;
        format!("All {} RUNNING tasks have been reset to PENDING", result.rows_affected())
    };

    Ok(Json(serde_json::json!({ "status": "success", "message": msg })))
}

// 6. drop all tasks
async fn clear_tasks(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    verify_token(&headers, &state.secret_token)?;

    sqlx::query("DELETE FROM tasks")
        .execute(&state.db)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { detail: "DB Error".into() })))?;

    Ok(Json(serde_json::json!({ "status": "success" })))
}
