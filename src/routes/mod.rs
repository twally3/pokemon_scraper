use std::collections::HashMap;

use app_state::AppState;
use askama::Template;
use axum::extract::{Path, Query, State};
use html_template::HtmlTemplate;
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

pub mod api;
pub mod app_state;
mod html_template;

#[derive(Template)]
#[template(path = "hello.html")]
struct HelloTemplate {
    name: String,
}

pub async fn greet(Path(name): Path<String>) -> impl axum::response::IntoResponse {
    let template = HelloTemplate { name };
    HtmlTemplate(template)
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum Class {
    Regular,
    ReverseHolo,
    Holo,
}

impl std::fmt::Display for Class {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Regular => "Regular",
                Self::ReverseHolo => "Reverse Holo",
                Self::Holo => "Holo",
            }
        )
    }
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

#[derive(Serialize, Deserialize, FromRow, Debug, Clone, PartialEq, Eq)]
struct Thing {
    id: u32,
    title: String,
    date: String,
    price: u32,
    link: String,
    bids: u32,
    accepts_offers: bool,
    offer_was_accepted: bool,
    card_set_name: String,
    card_expansion: u32,
    card_number: u32,
    card_class: Class,
    card_name: String,
    card_rarity: String,
}

impl PartialOrd for Thing {
    fn partial_cmp(&self, other: &Thing) -> Option<std::cmp::Ordering> {
        Some(self.price.cmp(&other.price))
    }
}

impl Ord for Thing {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.price.cmp(&other.price)
    }
}

impl From<Thing> for f64 {
    fn from(value: Thing) -> Self {
        value.price.into()
    }
}

struct Penis {
    price: f64,
    card_set_name: String,
    card_expansion: u32,
    card_number: u32,
    card_class: Class,
    card_name: String,
    card_rarity: String,
}

#[derive(Template)]
#[template(path = "main.html")]
struct MainTemplate {
    cards: Vec<Penis>,
}

pub async fn list_cards(
    Query(query_params): Query<HashMap<String, String>>,
    State(app_state): State<AppState>,
) -> impl axum::response::IntoResponse {
    let n = query_params
        .get("n")
        .unwrap_or(&String::from("30"))
        .parse::<u32>()
        .unwrap();

    let cards = sqlx::query_as::<_, Thing>(
        "
        SELECT *
        FROM ranked_listings 
        WHERE listing_rank <= ?
        ORDER BY card_expansion, card_number, card_class, listing_rank DESC;
        ",
    )
    .bind(n)
    .fetch_all(&app_state.pool)
    .await
    .expect("Failed to fetch cards");

    let x = cards
        .clone()
        .iter()
        .fold(HashMap::<_, Vec<Thing>>::new(), |mut acc, x| {
            acc.entry((x.card_expansion, x.card_number, x.card_class.to_string()))
                .and_modify(|a| a.push(x.clone()))
                .or_insert(vec![x.clone()]);

            acc
        })
        .into_iter()
        .map(|(k, v)| {
            let x = iqr(v);
            let count: u32 = x.len().try_into().unwrap();
            let sun: u32 = x.iter().map(|x| x.price).sum();
            (k, if count == 0 { 0 } else { sun / count })
        })
        .collect::<HashMap<_, _>>();

    let mut r = x
        .iter()
        .map(|((expansion, number, class), _)| {
            cards
                .iter()
                .find(|x| {
                    x.card_expansion == *expansion
                        && x.card_number == *number
                        && x.card_class.to_string() == *class
                })
                .cloned()
                .map(|x| Penis {
                    price: std::convert::Into::<f64>::into(x.price) / 100.0,
                    card_set_name: x.card_set_name,
                    card_expansion: x.card_expansion,
                    card_number: x.card_number,
                    card_class: x.card_class,
                    card_name: x.card_name,
                    card_rarity: x.card_rarity,
                })
                .expect("All varients should be in the HashMap")
        })
        .collect::<Vec<_>>();

    r.sort_by(|a, b| match a.card_set_name.cmp(&b.card_set_name) {
        std::cmp::Ordering::Equal => match a.card_expansion.cmp(&b.card_expansion) {
            std::cmp::Ordering::Equal => match a.card_number.cmp(&b.card_number) {
                std::cmp::Ordering::Equal => {
                    a.card_class.to_string().cmp(&b.card_class.to_string())
                }
                o => o,
            },
            o => o,
        },
        o => o,
    });

    let template = MainTemplate { cards: r };
    HtmlTemplate(template)
}

fn median<T>(xs: &[T]) -> Option<&T> {
    let len = xs.len();

    if len == 0 {
        return None;
    }

    Some(match len % 2 {
        // TODO: This should be (n + n+1)/2
        0 => &xs[(len + 1) / 2],
        1 => &xs[len / 2],
        _ => unreachable!(),
    })
}

fn iqr<T>(items: Vec<T>) -> Vec<T>
where
    T: Clone + Ord + Into<f64>,
{
    let n = match items.len() % 2 {
        0 => items.len() / 2,
        1 => (items.len() - 1) / 2,
        _ => unreachable!(),
    };

    let mut poo = items.clone();
    poo.sort();
    //poo.sort_by(|a, b| a.price.cmp(&b.price));

    let q1 = &poo[..n];
    let q3 = &poo[n..];

    let Some(q1) = median(q1) else {
        return vec![];
    };
    let Some(q3) = median(q3) else {
        return vec![];
    };

    let q1_price: f64 = q1.clone().into();
    let q3_price: f64 = q3.clone().into();

    let iqr = q3_price - q1_price;

    let lower_bound = q1_price - (iqr * 3.0 / 2.0);
    let upper_bound = q3_price + (iqr * 3.0 / 2.0);

    items
        .into_iter()
        .filter(|x| {
            std::convert::Into::<f64>::into(x.clone()) >= lower_bound
                && std::convert::Into::<f64>::into(x.clone()) <= upper_bound
        })
        .collect()
}

#[derive(Serialize, Deserialize, FromRow, Debug)]
struct Listing {
    id: u32,
    title: String,
    date: String,
    price: u32,
    link: String,
    bids: u32,
    accepts_offers: bool,
    offer_was_accepted: bool,
}

struct ViewListing {
    id: u32,
    title: String,
    date: String,
    price: f64,
    link: String,
    bids: u32,
    accepts_offers: bool,
    offer_was_accepted: bool,
}

impl From<Listing> for ViewListing {
    fn from(value: Listing) -> Self {
        ViewListing {
            id: value.id,
            title: value.title,
            date: value.date,
            price: std::convert::Into::<f64>::into(value.price) / 100.0,
            link: value.link,
            bids: value.bids,
            accepts_offers: value.accepts_offers,
            offer_was_accepted: value.offer_was_accepted,
        }
    }
}

#[derive(Template)]
#[template(path = "card.html")]
struct CardTemplate {
    listings: Vec<ViewListing>,
}

pub async fn card(
    Path((expansion, number, class)): Path<(u32, u32, String)>,
    State(app_state): State<AppState>,
) -> impl axum::response::IntoResponse {
    let listings = sqlx::query_as::<_, Listing>(
        "
        SELECT
            listings.*
        FROM
            listings_cards
            JOIN listings ON listings.id = listings_cards.listing_id
        WHERE
            listings_cards.card_set_name = \"Scarlet & Violet\"
            AND listings_cards.card_expansion = ?
            AND listings_cards.card_number = ?
            AND listings_cards.card_class = ?
        ORDER BY
            listings.date DESC;
        ",
    )
    .bind(expansion)
    .bind(number)
    .bind(class)
    .fetch_all(&app_state.pool)
    .await
    .expect("Failed to fetch cards");

    HtmlTemplate(CardTemplate {
        listings: listings.into_iter().map(|x| x.into()).collect(),
    })
}
