pub mod create;
pub mod delete;
pub mod jobs;
pub mod list;
pub mod log;
pub mod scale;

// re-exports crud functions
pub use create::*;
pub use delete::*;
pub use jobs::*;
pub use list::*;
pub use log::*;
pub use scale::*;

// use lazy_static::lazy_static;
use once_cell::sync::Lazy;
use reqwest::get;
use std::sync::Arc;
use tokio::sync::OnceCell;

static LOCAL_SERVER_URL: &str = "http://localhost:3000";
static REMOTE_SERVER_URL: &str = "http://3.132.162.86:30000";

static SERVER_URL: Lazy<OnceCell<Arc<String>>> = Lazy::new(|| OnceCell::new());

async fn lazy_load_server_url() -> Arc<String> {
    // Try connecting to the local server first
    if is_server_available(LOCAL_SERVER_URL).await {
        println!("Connected to local server: {}", LOCAL_SERVER_URL);
        return Arc::new(LOCAL_SERVER_URL.to_string());
    }

    // Try connecting to the remote server if the local one is unavailable
    if is_server_available(REMOTE_SERVER_URL).await {
        println!("Connected to remote server: {}", REMOTE_SERVER_URL);
        return Arc::new(REMOTE_SERVER_URL.to_string());
    }

    // Panic if neither server is reachable
    panic!("No server available: could not connect to either local or remote server");
}

async fn is_server_available(url: &str) -> bool {
    match get(url).await {
        Ok(response) => response.status().is_success(),
        Err(_) => false,
    }
}

async fn get_server_url() -> Arc<String> {
    SERVER_URL
        .get_or_init(|| async { lazy_load_server_url().await })
        .await
        .clone()
}
