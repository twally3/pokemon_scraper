#![warn(missing_debug_implementations, rust_2018_idioms, rustdoc::all)]

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use thirtyfour::*;

const PAGINATION_LIMIT: usize = 100;

#[derive(Debug, Deserialize)]
enum Rarity {
    #[serde(rename = "https://tcgcodex.com/images/rarities/pokemon-common.webp")]
    Common,
    #[serde(rename = "https://tcgcodex.com/images/rarities/pokemon-uncommon.webp")]
    Uncommon,
    #[serde(rename = "https://tcgcodex.com/images/rarities/pokemon-rare.webp")]
    Rare,
    #[serde(rename = "https://tcgcodex.com/images/rarities/pokemon-double-rare.webp")]
    DoubleRare,
    #[serde(rename = "https://tcgcodex.com/images/rarities/pokemon-ace-spec-rare.webp")]
    AceSpecRare,
    #[serde(rename = "https://tcgcodex.com/images/rarities/pokemon-illustration-rare.webp")]
    IllustrationRare,
    #[serde(rename = "https://tcgcodex.com/images/rarities/pokemon-ultra-rare.webp")]
    UltraRare,
    #[serde(
        rename = "https://tcgcodex.com/images/rarities/pokemon-special-illustration-rare.webp"
    )]
    SpecialIllustrationRare,
    #[serde(rename = "https://tcgcodex.com/images/rarities/pokemon-hyper-rare.webp")]
    HyperRare,
}

impl std::fmt::Display for Rarity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Common => "Common",
                Self::Uncommon => "Uncommon",
                Self::Rare => "Rare",
                Self::DoubleRare => "Double Rare",
                Self::AceSpecRare => "Ace Spec Rare",
                Self::IllustrationRare => "Illustration Rare",
                Self::UltraRare => "Ultra Rare",
                Self::SpecialIllustrationRare => "Special Illustration Rare",
                Self::HyperRare => "Hyper Rare",
            }
        )
    }
}

#[derive(Debug, Deserialize)]
enum Class {
    #[serde(rename = "advanced-pkmn_regular-unchecked")]
    Regular,
    #[serde(rename = "advanced-pkmn_rev_holo-unchecked")]
    ReverseHolo,
    #[serde(rename = "advanced-pkmn_foil-unchecked")]
    Foil,
}

impl std::fmt::Display for Class {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Regular => "Regular",
                Self::ReverseHolo => "Reverse Holo",
                Self::Foil => "Holo",
            }
        )
    }
}

#[derive(Debug, Serialize)]
#[allow(dead_code)]
enum BuyingFormat {
    // TODO: Look into accepts_offers
    Auction {
        bids: usize,
        offer_was_accepted: bool,
    },
    BuyItNow {
        accepts_offers: bool,
        offer_was_accepted: bool,
    },
}

impl BuyingFormat {
    fn get_bids(&self) -> Option<usize> {
        match self {
            Self::Auction { bids, .. } => Some(*bids),
            _ => None,
        }
    }

    fn get_accepts_offers(&self) -> Option<bool> {
        match self {
            Self::BuyItNow { accepts_offers, .. } => Some(*accepts_offers),
            _ => None,
        }
    }

    fn get_offer_was_accepted(&self) -> bool {
        match self {
            Self::BuyItNow {
                offer_was_accepted, ..
            } => *offer_was_accepted,
            Self::Auction {
                offer_was_accepted, ..
            } => *offer_was_accepted,
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
struct Listing {
    id: usize,
    title: String,
    date: NaiveDate,
    price: f32,
    link: String,
    buying_format: BuyingFormat,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Pokemon {
    name: String,
    number: usize,
    rarity: Rarity,
    class: Class,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Expansion {
    set_name: String,
    expansion_name: String,
    expansion_number: usize,
    expansion_total: usize,
    cards: Vec<Pokemon>,
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
        .max_connections(1)
        .connect_with(connection_options)
        .await?;

    sqlx::migrate!("db/migrations").run(&pool).await?;

    expansion
        .cards
        .iter()
        .fold(
            sqlx::query(&format!(
                "INSERT INTO cards (number, class, name, rarity) VALUES {} ON CONFLICT DO NOTHING",
                expansion
                    .cards
                    .iter()
                    .map(|_| "(?,?,?,?)")
                    .collect::<Vec<_>>()
                    .join(",")
            )),
            |acc, x| {
                acc.bind(x.number as u32)
                    .bind(x.class.to_string())
                    .bind(x.name.clone())
                    .bind(x.rarity.to_string())
            },
        )
        .execute(&pool)
        .await?;

    //let cards = expansion.cards.into_iter();
    //let cards = expansion.cards.into_iter().take(191);
    let cards = expansion.cards.into_iter().take_while(|x| x.number <= 191);
    //let cards = expansion.cards.into_iter().filter(|x| x.number == 238);
    //let cards = expansion.cards.into_iter().filter(|x| x.number == 1);
    //let cards = expansion.cards.into_iter().filter(|x| x.number == 4);
    //let cards = expansion
    //    .cards
    //    .into_iter()
    //    .filter(|x| x.number > expansion.expansion_total)
    //    .take(5);
    let cards = cards.collect::<Vec<_>>();

    let mut caps = DesiredCapabilities::chrome();
    caps.add_arg("--start-maximized")?;
    let driver = WebDriver::new("http://localhost:9515", caps).await?;

    for card in cards {
        let last_listing_date = sqlx::query_as::<_,  (chrono::NaiveDate, )>("SELECT date FROM listings WHERE card_number = ? AND card_class = ? ORDER BY date DESC LIMIT 1")
            .bind(card.number as u32)
            .bind(card.class.to_string())
            .fetch_optional(&pool)
            .await?
            .map(|x| x.0);

        // TODO: Consider clearing the textbox
        driver.goto("https://ebay.co.uk").await?;

        println!("{card:#?}");

        driver
            .find(By::Id("gh-ac"))
            .await?
            .send_keys(format!(
                "{} {:0>3}/{}",
                card.name, card.number, expansion.expansion_total
            ))
            .await?;

        match driver.find(By::Id("gh-btn")).await {
            btn @ Ok(_) => btn,
            Err(_) => match driver.find(By::Id("gh-search-btn")).await {
                btn @ Ok(_) => btn,
                err => err,
            },
        }?
        .click()
        .await?;

        // Change page count to 240
        if let Some(url) = match driver
            .find(By::Css("#srp-ipp-menu-content li:last-child a"))
            .await
        {
            Ok(x) => x.prop("href").await,
            Err(err) => match err.as_inner() {
                error::WebDriverErrorInner::NoSuchElement(_) => Ok(None),
                _ => Err(err),
            },
        }? {
            driver.goto(url).await?;
        }

        driver
            .find(By::Css("input[type=checkbox][aria-label='Sold items']"))
            .await?
            .click()
            .await?;

        if let Ok(grade_btn) = driver
            .find(By::Css(
                "li[name=Grade] input[type=checkbox][aria-label='Not specified']",
            ))
            .await
        {
            grade_btn.click().await?;
        }

        let mut final_listings = Vec::new();

        let mut page_count = 0;
        'outer: loop {
            let listings = driver.find_all(By::Css("ul.srp-results > li")).await?;

            for listing in listings {
                let Some(class_names) = listing.class_name().await? else {
                    println!("Couldn't find class_names for listing");
                    continue;
                };

                if !class_names.split_whitespace().any(|x| x == "s-item") {
                    if listing.text().await? == "Results matching fewer words" {
                        println!("Reached end of good results");
                        break;
                    };

                    continue;
                }

                let date = NaiveDate::parse_from_str(
                    listing
                        .find(By::Css(".s-item__caption"))
                        .await?
                        .text()
                        .await?
                        .trim_start_matches("Sold "),
                    "%-d %b %Y",
                )?;

                if last_listing_date.map(|d| date < d).unwrap_or(false) {
                    println!(
                        "Listing date {date} is less than last recorded date {}. Ending.",
                        last_listing_date.unwrap()
                    );
                    break 'outer;
                }

                let title = listing
                    .find(By::Css("a.s-item__link span[role=heading]"))
                    .await?
                    .text()
                    .await?;

                if !std::iter::once(card.name.to_lowercase().as_str())
                    .chain(card.name.to_lowercase().split_whitespace())
                    .any(|x| title.to_lowercase().contains(x))
                {
                    println!("Title \"{}\" doesn't contain card name. Skipping.", title);
                    continue;
                }

                if match card.class {
                    Class::Regular => ["reverse holo", "reverse"]
                        .into_iter()
                        .any(|x| title.to_lowercase().contains(x)),
                    Class::ReverseHolo => title.to_lowercase().contains("regular"),
                    Class::Foil => false,
                } {
                    println!("Title \"{}\" contains blacklisted words. Skipping.", title);
                    continue;
                }

                if match card.class {
                    Class::Regular => false,
                    Class::ReverseHolo => !["reverse holo", "holo", "reverse"]
                        .into_iter()
                        .any(|x| title.to_lowercase().contains(x)),
                    Class::Foil => false,
                } {
                    println!(
                        "Title \"{}\" doesn't contain whitelisted words. Skipping",
                        title
                    );
                    continue;
                }

                let price = listing
                    .find(By::Css(".s-item__price"))
                    .await?
                    .text()
                    .await?;

                // TODO: Using floats for money is sinful
                let Ok(price) = price
                    .trim_start_matches("£")
                    .replace(",", "")
                    .parse::<f32>()
                else {
                    println!("Failed to parse price {price}. Skipping.");
                    continue;
                };

                let link = listing
                    .find(By::Css(".s-item__link"))
                    .await?
                    .prop("href")
                    .await?
                    .unwrap_or("".into());

                let link = link
                    .split("?")
                    .next()
                    .expect("One result should always be returned")
                    .to_string();

                let id = link
                    .split("/")
                    .last()
                    .expect("Split always returns one value")
                    .parse()?;

                let buying_format =
                    if let Ok(bids) = listing.find(By::Css(".s-item__bidCount")).await {
                        Ok(BuyingFormat::Auction {
                            bids: bids
                                .text()
                                .await?
                                .split_whitespace()
                                .next()
                                .expect("Should always have a value")
                                .parse()
                                .expect("Should always be a number"),
                            offer_was_accepted: listing
                                .find(By::Css(".s-item__formatBestOfferAccepted"))
                                .await
                                .map(|_| true)
                                .unwrap_or(false),
                        })
                    } else if listing
                        .find(By::Css(".s-item__formatBuyItNow"))
                        .await
                        .is_ok()
                    {
                        Ok(BuyingFormat::BuyItNow {
                            accepts_offers: false,
                            offer_was_accepted: false,
                        })
                    } else if listing
                        .find(By::Css(".s-item__formatBestOfferEnabled"))
                        .await
                        .is_ok()
                    {
                        Ok(BuyingFormat::BuyItNow {
                            accepts_offers: true,
                            offer_was_accepted: false,
                        })
                    } else if listing
                        .find(By::Css(".s-item__formatBestOfferAccepted"))
                        .await
                        .is_ok()
                    {
                        Ok(BuyingFormat::BuyItNow {
                            accepts_offers: true,
                            offer_was_accepted: true,
                        })
                    } else {
                        Err("Failed to resolve buying format")
                    }?;

                let listing = Listing {
                    id,
                    title,
                    date,
                    price,
                    link,
                    buying_format,
                };

                final_listings.push(listing);
            }

            match driver.find(By::Css("a.pagination__next")).await {
                btn @ Ok(_) => btn,
                Err(err) => match err.as_inner() {
                    error::WebDriverErrorInner::NoSuchElement(_) => break,
                    _ => Err(err),
                },
            }?
            .click()
            .await?;

            page_count += 1;
            if page_count > PAGINATION_LIMIT {
                println!("PAGINATION LIMIT");
                break;
            }
        }

        if final_listings.is_empty() {
            continue;
        }

        let query_string = format!(
                "INSERT INTO listings (id, title, date, price, link, bids, accepts_offers, offer_was_accepted, card_number, card_class) VALUES {} ON CONFLICT DO NOTHING",
                final_listings
                    .iter()
                    .map(|_| "(?,?,?,?,?,?,?,?,?,?)")
                    .collect::<Vec<_>>()
                    .join(",")
            );

        final_listings
            .iter()
            .fold(sqlx::query(&query_string), |acc, x| {
                acc.bind(x.id as u32)
                    .bind(x.title.clone())
                    .bind(x.date)
                    .bind(x.price)
                    .bind(x.link.clone())
                    .bind(x.buying_format.get_bids().map(|x| x as u32))
                    .bind(x.buying_format.get_accepts_offers())
                    .bind(x.buying_format.get_offer_was_accepted())
                    .bind(card.number as u32)
                    .bind(card.class.to_string())
            })
            .execute(&pool)
            .await?;

        //std::thread::sleep(std::time::Duration::new(10, 0));
    }

    Ok(())
}
