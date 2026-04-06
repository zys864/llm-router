use std::net::SocketAddr;

use llm_router::{build_app, config::AppConfig};
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let config = AppConfig::from_env().expect("failed to load config");
    let addr: SocketAddr = config.bind_addr.parse().expect("invalid bind address");

    let app = build_app(config);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind tcp listener");

    info!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.expect("server failed");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn app_builds_without_panicking() {
        let _ = build_app(AppConfig::default());
    }
}
