#![warn(missing_debug_implementations, rust_2018_idioms, rustdoc::all)]

use card_scraper::{CardScaper, Expansion};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use thirtyfour::*;

mod card_scraper;
mod currency;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let expansion = include_str!("../pokemon.json");
    let expansion = serde_json::from_str::<Expansion>(expansion)?;

    let connection_options = SqliteConnectOptions::new()
        .filename("db/demo.db")
        .foreign_keys(true)
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(connection_options)
        .await?;

    sqlx::migrate!("db/migrations").run(&pool).await?;

    let wd_url = std::env::var("WEB_DRIVER_URL").unwrap_or("http://localhost:9515".into());
    let mut caps = DesiredCapabilities::chrome();
    caps.add_arg("--start-maximized")?;
    let driver = WebDriver::new(wd_url, caps).await?;

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(());
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    let scraper = CardScaper::new(pool.clone(), driver, shutdown_rx);

    let h = tokio::spawn(async move { scraper.scrape_expansion(expansion).await });

    tokio::select! {
        _ = ctrl_c => {
            println!("Initiating shutdown");
            shutdown_tx.send(()).expect("Failed to send shutdown signal");

            h.await??;
            println!("Scraper shutdown completed");
        }
        //err = server => {
        //    println!("Server error: {:?}", err);
        //}
    }

    Ok(())
}
