#![warn(missing_debug_implementations, rust_2018_idioms, rustdoc::all)]

use axum::{extract::State, Json};
use card_scraper::{CardScaper, Expansion};
use serde::Serialize;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Sqlite,
};
use thirtyfour::*;

mod card_scraper;
mod currency;

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

#[derive(Clone, Debug)]
struct AppState {
    pool: sqlx::Pool<Sqlite>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let expansion = include_str!("../pokemon.json");
    let expansion = serde_json::from_str::<Expansion>(expansion)?;

    let connection_options = SqliteConnectOptions::new()
        .filename("db/demo.db")
        .foreign_keys(true)
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .connect_with(connection_options)
        .await?;

    sqlx::migrate!("db/migrations").run(&pool).await?;

    let wd_url = std::env::var("WEB_DRIVER_URL").unwrap_or("http://localhost:9515".into());
    let mut caps = DesiredCapabilities::chrome();
    caps.add_arg("--start-maximized")?;
    let driver = WebDriver::new(wd_url, caps).await?;

    let (scraper_tx, mut scraper_rx) = tokio::sync::watch::channel(());
    let (server_tx, mut server_rx) = tokio::sync::watch::channel(());

    let shutdown = std::sync::Arc::new(tokio::sync::Notify::new());
    let shutdown_scraper = shutdown.clone();
    let shutdown_server = shutdown.clone();

    let scraper = CardScaper::new(pool.clone(), driver, shutdown_scraper);
    let h = tokio::spawn(async move {
        let a = scraper.scrape_expansion(expansion).await;
        scraper_tx.send(()).expect("Failed to send scraper signal");
        a
    });

    let app = axum::Router::new()
        .route("/", axum::routing::get(root))
        .with_state(AppState { pool });

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    let server = axum::serve(listener, app).with_graceful_shutdown(async move {
        shutdown_server.notified().await;
    });

    let server = tokio::spawn(async move {
        let _ = server.await;
        server_tx.send(()).expect("Failed to send server signal");
    });

    let mut a = scraper_rx.clone();
    let mut b = server_rx.clone();

    tokio::select! {
        _ = shutdown_signal() => {
            println!("Shutdown signal received");
            shutdown.notify_waiters();
        }
        _ = a.changed() => {
            println!("Scraper completed");
            shutdown.notify_waiters();
        }
        _ = b.changed() => {
            println!("Server completed");
            shutdown.notify_waiters();
        }
    }

    let _ = tokio::join!(server, h, scraper_rx.changed(), server_rx.changed());

    Ok(())
}

#[derive(Serialize)]
struct Test {
    hello: String,
}

async fn root(State(app_state): State<AppState>) -> Json<Test> {
    let t = Test {
        hello: "world".into(),
    };
    Json(t)
}
