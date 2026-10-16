//! Builds the axum Router (shared by `main` and the integration tests) and seeds the admin account.
use crate::routes;
use crate::state::AppState;
use axum::routing::get;
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;

pub fn build(state: AppState) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .nest("/api/v1", routes::client::router())
        .nest("/api/v1/admin", routes::admin::router())
        .fallback_service(tower_http::services::ServeDir::new("web"))
        .layer(RequestBodyLimitLayer::new(16 * 1024 * 1024))
        .layer(CorsLayer::very_permissive())
        .with_state(state)
}

/// Creates the admin account on first run from env (or a printed random password).
pub fn seed_admin(state: &AppState) {
    let Ok(c) = state.pool.get() else { return };
    let have: i64 = c.query_row("SELECT count(*) FROM users WHERE role='admin'", [], |r| r.get(0)).unwrap_or(0);
    if have > 0 {
        return;
    }
    let user = std::env::var("HAMISTA_ADMIN_USER").unwrap_or_else(|_| "admin".into());
    let pass = std::env::var("HAMISTA_ADMIN_PASS").unwrap_or_else(|_| crate::crypto::random_token()[..12].to_string());
    match crate::crypto::hash_password(&pass) {
        Ok(hash) => {
            let _ = c.execute(
                "INSERT INTO users(username,pass_hash,role,customer_name) VALUES(?1,?2,'admin','مدیر')",
                rusqlite::params![user, hash],
            );
            let note = format!("admin account\nusername: {user}\npassword: {pass}\n(change it after first login)\n");
            let _ = std::fs::write(state.cfg.data_dir.join("admin-credentials.txt"), &note);
            tracing::warn!("ساخت حساب مدیر — نام کاربری: {user} — رمز در data_dir/admin-credentials.txt");
        }
        Err(e) => tracing::error!("seed admin: {}", e.message),
    }
}
