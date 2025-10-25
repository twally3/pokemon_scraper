#![warn(missing_debug_implementations, rust_2018_idioms, rustdoc::all)]

use card_scraper::{CardScaper, Expansion};
use routes::{app_state::AppState, card, greet, list_cards};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use thirtyfour::*;

mod card_scraper;
mod currency;
mod routes;

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let expansions = vec![
        include_str!("../expansions/shrouded_fable.json"),
        include_str!("../expansions/destined_rivals.json"),
        include_str!("../expansions/stellar_crown.json"),
        include_str!("../expansions/obsidian_flames.json"),
        include_str!("../expansions/temporal_forces.json"),
        include_str!("../expansions/surging_sparks.json"),
        include_str!("../expansions/mega_evolution.json"),
    ];

    let expansions = expansions
        .into_iter()
        .map(serde_json::from_str::<Expansion>)
        .collect::<Result<Vec<_>, _>>()?;

    let connection_options = SqliteConnectOptions::new()
        .filename("db/demo.db")
        .foreign_keys(true)
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .connect_with(connection_options)
        .await?;

    sqlx::migrate!("db/migrations").run(&pool).await?;

    let wd_url = std::env::var("WEB_DRIVER_URL").unwrap_or("http://localhost:4444".into());
    let sleep_secs = std::env::var("SCRAPER_SLEEP_SECS")
        .unwrap_or("20".into())
        .parse::<u64>()
        .expect("Failed to parse SCRAPER_SLEEP_SECS");
    let mut caps = DesiredCapabilities::chrome();
    caps.add_arg("--start-maximized")?;

    let (scraper_tx, mut scraper_rx) = tokio::sync::watch::channel(());
    let (server_tx, mut server_rx) = tokio::sync::watch::channel(());

    let shutdown = std::sync::Arc::new(tokio::sync::Notify::new());
    let shutdown_scraper = shutdown.clone();
    let shutdown_server = shutdown.clone();

    let scraper = CardScaper::new(pool.clone(), wd_url, caps, shutdown_scraper, sleep_secs);

    let h = tokio::spawn(async move {
        let a = scraper.start_scraping_expansions(expansions).await;
        scraper_tx.send(()).expect("Failed to send scraper signal");
        a
    });

    let api_routes = axum::Router::new().route("/", axum::routing::get(routes::api::say_hello));

    let app = axum::Router::new()
        .nest("/api", api_routes)
        .route("/greet/{name}", axum::routing::get(greet))
        .route("/", axum::routing::get(list_cards))
        .route("/{expansion}/{number}/{class}", axum::routing::get(card))
        .with_state(AppState { pool });

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    let server = axum::serve(listener, app).with_graceful_shutdown(async move {
        shutdown_server.notified().await;
    });

    let server = tokio::spawn(async move {
        let a = server.await;
        server_tx.send(()).expect("Failed to send server signal");
        a
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

    let (h, server, ..) = tokio::join!(h, server, scraper_rx.changed(), server_rx.changed());

    match h {
        Err(e) => Err(e.to_string()),
        Ok(Err(e)) => Err(e.to_string()),
        _ => Ok(()),
    }?;

    match server {
        Err(e) => Err(e.to_string()),
        Ok(Err(e)) => Err(e.to_string()),
        _ => Ok(()),
    }?;

    Ok(())
}
