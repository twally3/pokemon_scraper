#![warn(missing_debug_implementations, rust_2018_idioms, rustdoc::all)]

use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use card_scraper::{CardScaper, Expansion};
use serde::{Deserialize, Serialize};
use sqlx::{
    prelude::FromRow,
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

    let wd_url = std::env::var("WEB_DRIVER_URL").unwrap_or("http://localhost:4444".into());
    let sleep_secs = std::env::var("SCRAPER_SLEEP_SECS")
        .unwrap_or("20".into())
        .parse::<u64>()
        .expect("Failed to parse SCRAPER_SLEEP_SECS");
    let mut caps = DesiredCapabilities::chrome();
    caps.add_arg("--start-maximized")?;
    let driver = WebDriver::new(wd_url, caps).await?;

    let (scraper_tx, mut scraper_rx) = tokio::sync::watch::channel(());
    let (server_tx, mut server_rx) = tokio::sync::watch::channel(());

    let shutdown = std::sync::Arc::new(tokio::sync::Notify::new());
    let shutdown_scraper = shutdown.clone();
    let shutdown_server = shutdown.clone();

    let scraper = CardScaper::new(pool.clone(), driver, shutdown_scraper, sleep_secs);
    let h = tokio::spawn(async move {
        let a = scraper.scrape_expansion(expansion).await;
        scraper_tx.send(()).expect("Failed to send scraper signal");
        a
    });

    let api_routes = axum::Router::new()
        .route("/cards", axum::routing::get(get_cards))
        .route(
            "/cards/{card_number}",
            axum::routing::get(get_cards_by_number),
        )
        .route(
            "/cards/{card_number}/{card_class}",
            axum::routing::get(get_card),
        )
        .route(
            "/cards/{card_number}/{card_class}/listings",
            axum::routing::get(get_listings_for_card),
        )
        .route(
            "/cards/{card_number}/{card_class}/listings/iqr",
            axum::routing::get(get_listings_for_card_iqr),
        )
        .with_state(AppState { pool });

    let app = axum::Router::new().nest("/api", api_routes);

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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Class {
    Regular,
    ReverseHolo,
    Holo,
}

impl sqlx::Type<sqlx::Sqlite> for Class {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <String as sqlx::Type<sqlx::Sqlite>>::type_info()
    }
}

impl sqlx::Decode<'_, sqlx::Sqlite> for Class {
    fn decode(value: sqlx::sqlite::SqliteValueRef<'_>) -> Result<Self, sqlx::error::BoxDynError> {
        let value = <String as sqlx::Decode<sqlx::Sqlite>>::decode(value)?;
        match value.as_str() {
            "Regular" => Ok(Class::Regular),
            "Reverse Holo" => Ok(Class::ReverseHolo),
            "Holo" => Ok(Class::Holo),
            _ => Err("Invalid class variant".into()),
        }
    }
}

#[derive(Serialize, Deserialize, FromRow, Clone, Debug)]
struct Listing {
    id: u32,
    title: String,
    date: String,
    price: u32,
    link: String,
    bids: u32,
    accepts_offers: bool,
    offer_was_accepted: bool,
    card_number: u32,
    card_class: Class,
}

#[derive(Serialize, Deserialize, FromRow)]
struct Card {
    number: u32,
    class: Class,
    name: String,
    rarity: String,
}

async fn get_cards(State(app_state): State<AppState>) -> Json<Vec<Card>> {
    let cards = sqlx::query_as::<_, Card>("SELECT * FROM cards")
        .fetch_all(&app_state.pool)
        .await
        .expect("Failed to fetch cards");

    Json(cards)
}

async fn get_cards_by_number(
    Path(card_number): Path<u32>,
    State(app_state): State<AppState>,
) -> Json<Vec<Card>> {
    let cards = sqlx::query_as::<_, Card>("SELECT * FROM cards WHERE number = ?")
        .bind(card_number)
        .fetch_all(&app_state.pool)
        .await
        .expect("Failed to fetch cards");

    Json(cards)
}

async fn get_card(
    Path((card_number, card_class)): Path<(u32, String)>,
    State(app_state): State<AppState>,
) -> Json<Card> {
    let cards = sqlx::query_as::<_, Card>("SELECT * FROM cards WHERE number = ? AND class = ?")
        .bind(card_number)
        .bind(card_class)
        .fetch_one(&app_state.pool)
        .await
        .expect("Failed to fetch cards");

    Json(cards)
}

async fn get_listings_for_card(
    Path((card_id, card_class)): Path<(u32, String)>,
    State(app_state): State<AppState>,
) -> Json<Vec<Listing>> {
    let listings = sqlx::query_as::<_, Listing>(
        "SELECT listings.* FROM cards JOIN listings ON listings.card_number = cards.number AND listings.card_class = cards.class WHERE cards.number = ? AND cards.class = ?",
    )
    .bind(card_id)
    .bind(card_class)
    .fetch_all(&app_state.pool)
    .await
    .expect("Failed to fetch listings");

    Json(listings)
}

fn median<T>(xs: &[T]) -> &T {
    let len = xs.len();

    match len % 2 {
        // TODO: This should be (n + n+1)/2
        0 => &xs[(len + 1) / 2],
        1 => &xs[len / 2],
        _ => unreachable!(),
    }
}

#[derive(Serialize)]
struct Anus {
    lb: f64,
    ub: f64,
    q1: Vec<Listing>,
    q2: Vec<Listing>,
    q3: Vec<Listing>,
}

async fn get_listings_for_card_iqr(
    Path((card_id, card_class)): Path<(u32, String)>,
    Query(query_params): Query<HashMap<String, String>>,
    State(app_state): State<AppState>,
) -> Json<Anus> {
    let listings = if let Some(max_date) = query_params.get("max_date") {
        sqlx::query_as::<_, Listing>(
            "SELECT listings.* FROM cards JOIN listings ON listings.card_number = cards.number AND listings.card_class = cards.class WHERE cards.number = ? AND cards.class = ? AND listings.date >= ?",
        )
            .bind(card_id)
            .bind(card_class)
            .bind(max_date)
            .fetch_all(&app_state.pool)
            .await
            .expect("Failed to fetch listings")
    } else {
        sqlx::query_as::<_, Listing>(
            "SELECT listings.* FROM cards JOIN listings ON listings.card_number = cards.number AND listings.card_class = cards.class WHERE cards.number = ? AND cards.class = ?",
        )
            .bind(card_id)
            .bind(card_class)
            .fetch_all(&app_state.pool)
            .await
            .expect("Failed to fetch listings")
    };

    let n = match listings.len() % 2 {
        0 => listings.len() / 2,
        1 => (listings.len() - 1) / 2,
        _ => unreachable!(),
    };

    let mut poo = listings.clone();
    poo.sort_by(|a, b| a.price.cmp(&b.price));

    let q1 = &poo[..n];
    let q3 = &poo[n..];

    let q1 = median(q1);
    let q3 = median(q3);

    let q1_price: f64 = q1.price.into();
    let q3_price: f64 = q3.price.into();

    let iqr = q3_price - q1_price;

    let lower_bound = q1_price - (iqr * 3.0 / 2.0);
    let upper_bound = q3_price + (iqr * 3.0 / 2.0);

    dbg!(lower_bound, upper_bound);

    let mut lower_shit = listings
        .iter()
        .filter(|x| std::convert::Into::<f64>::into(x.price) < lower_bound)
        .cloned()
        .collect::<Vec<_>>();

    let mut upper_shit = listings
        .iter()
        .filter(|x| std::convert::Into::<f64>::into(x.price) > upper_bound)
        .cloned()
        .collect::<Vec<_>>();

    let mut middle_shit = listings
        .iter()
        .filter(|x| {
            std::convert::Into::<f64>::into(x.price) >= lower_bound
                && std::convert::Into::<f64>::into(x.price) <= upper_bound
        })
        .cloned()
        .collect::<Vec<_>>();

    lower_shit.sort_by(|a, b| a.price.cmp(&b.price));
    middle_shit.sort_by(|a, b| a.price.cmp(&b.price));
    upper_shit.sort_by(|a, b| a.price.cmp(&b.price));

    Json(Anus {
        lb: lower_bound,
        ub: upper_bound,
        q1: lower_shit,
        q2: middle_shit,
        q3: upper_shit,
    })
}
