//! هامیستا کلود — central server: licensing, logs, mega-bank, notifications, versions, scenarios.
use hamista_cloud::app;
use hamista_cloud::state::{AppState, Config};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,hamista_cloud=info".into()))
        .init();

    let cfg = Config::from_env();
    let bind = cfg.bind.clone();
    let state = AppState::new(cfg).unwrap_or_else(|e| {
        eprintln!("خطای راه‌اندازی: {}", e.message);
        std::process::exit(1);
    });
    app::seed_admin(&state);

    let app = app::build(state);
    let listener = tokio::net::TcpListener::bind(&bind).await.unwrap_or_else(|e| {
        eprintln!("نمی‌توان روی {bind} گوش داد: {e}");
        std::process::exit(1);
    });
    tracing::info!("هامیستا کلود روی http://{bind} اجرا شد");
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(async { let _ = tokio::signal::ctrl_c().await; })
        .await
        .unwrap();
}
