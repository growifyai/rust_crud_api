use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use dotenv::dotenv;
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::env;
use std::net::SocketAddr;
use tokio;

// The data model for a Todo item.
// `serde` is used for serializing and deserializing JSON.
// `sqlx::FromRow` allows us to map database rows to this struct.
#[derive(Serialize, Deserialize, sqlx::FromRow, Clone)]
struct Todo {
    id: i32,
    title: String,
    completed: bool,
}

// The data model for creating a new Todo item.
// We don't need an `id` when creating a new item, as the database will generate it.
#[derive(Deserialize)]
struct CreateTodo {
    title: String,
}

// The application state, which holds the database connection pool.
// We use `Clone` so that the state can be shared across handlers.
#[derive(Clone)]
struct AppState {
    pool: PgPool,
}

#[tokio::main]
async fn main() {
    // Load environment variables from a .env file. This is for local development.
    // Render will use its own environment variable management.
    dotenv().ok();

    // Get the database URL from the environment variables.
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    // Create a PostgreSQL connection pool.
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to create pool.");

    // Create the application state.
    let app_state = AppState { pool };

    // Create the axum router.
    let app = Router::new()
        .route("/todos", post(create_todo).get(get_todos))
        .route("/todos/:id", get(get_todo).put(update_todo).delete(delete_todo))
        .with_state(app_state);

    // Get the port from the environment or default to 3000.
    // Render sets the PORT environment variable.
    let port = env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse::<u16>()
        .expect("PORT must be a valid number");
        
    // Define the server address. We need to bind to 0.0.0.0 for Render.
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("listening on {}", addr);

    // Start the server.
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

// API handler to create a new todo item.
async fn create_todo(
    State(state): State<AppState>,
    Json(payload): Json<CreateTodo>,
) -> impl IntoResponse {
    let result = sqlx::query_as::<_, Todo>(
        "INSERT INTO todos (title) VALUES ($1) RETURNING id, title, completed",
    )
    .bind(payload.title)
    .fetch_one(&state.pool)
    .await;

    match result {
        Ok(todo) => (StatusCode::CREATED, Json(todo)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create todo: {}", e),
        )
            .into_response(),
    }
}

// API handler to get all todo items.
async fn get_todos(State(state): State<AppState>) -> impl IntoResponse {
    let result = sqlx::query_as::<_, Todo>("SELECT id, title, completed FROM todos")
        .fetch_all(&state.pool)
        .await;

    match result {
        Ok(todos) => (StatusCode::OK, Json(todos)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to fetch todos: {}", e),
        )
            .into_response(),
    }
}

// API handler to get a single todo item by its ID.
async fn get_todo(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    let result = sqlx::query_as::<_, Todo>("SELECT id, title, completed FROM todos WHERE id = $1")
        .bind(id)
        .fetch_one(&state.pool)
        .await;

    match result {
        Ok(todo) => (StatusCode::OK, Json(todo)).into_response(),
        Err(sqlx::Error::RowNotFound) => {
            (StatusCode::NOT_FOUND, format!("Todo with id {} not found", id)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to fetch todo: {}", e),
        )
            .into_response(),
    }
}

// API handler to update a todo item.
async fn update_todo(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(payload): Json<CreateTodo>,
) -> impl IntoResponse {
    let result = sqlx::query_as::<_, Todo>(
        "UPDATE todos SET title = $1, completed = false WHERE id = $2 RETURNING id, title, completed",
    )
    .bind(payload.title)
    .bind(id)
    .fetch_one(&state.pool)
    .await;

    match result {
        Ok(todo) => (StatusCode::OK, Json(todo)).into_response(),
        Err(sqlx::Error::RowNotFound) => {
            (StatusCode::NOT_FOUND, format!("Todo with id {} not found", id)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to update todo: {}", e),
        )
            .into_response(),
    }
}

// API handler to delete a todo item.
async fn delete_todo(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    let result = sqlx::query("DELETE FROM todos WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await;

    match result {
        Ok(res) if res.rows_affected() > 0 => (StatusCode::NO_CONTENT).into_response(),
        Ok(_) => (StatusCode::NOT_FOUND, format!("Todo with id {} not found", id)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to delete todo: {}", e),
        )
            .into_response(),
    }
}
