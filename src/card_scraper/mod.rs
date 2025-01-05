use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::Sqlite;
use thirtyfour::{By, WebDriver};

use crate::currency::{Money, GBP};

const PAGINATION_LIMIT: usize = 100;

#[derive(Debug, Deserialize, Clone)]
pub enum Rarity {
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

#[derive(Debug, Deserialize, Clone)]
pub enum Class {
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
struct Listing<'a> {
    id: usize,
    title: String,
    date: NaiveDate,
    price: Money<'a>,
    link: String,
    buying_format: BuyingFormat,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct Pokemon {
    pub name: String,
    pub number: usize,
    pub rarity: Rarity,
    pub class: Class,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct Expansion {
    pub set_name: String,
    pub expansion_name: String,
    pub expansion_number: usize,
    pub expansion_total: usize,
    pub cards: Vec<Pokemon>,
}

pub struct CardScaper {
    pool: sqlx::Pool<Sqlite>,
    driver: WebDriver,
    shutdown_rx: tokio::sync::watch::Receiver<()>,
}

impl CardScaper {
    pub fn new(
        pool: sqlx::Pool<Sqlite>,
        driver: thirtyfour::WebDriver,
        shutdown_rx: tokio::sync::watch::Receiver<()>,
    ) -> Self {
        Self {
            pool,
            driver,
            shutdown_rx,
        }
    }

    pub async fn scrape_expansion(&self, expansion: Expansion) -> Result<(), String> {
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
            .execute(&self.pool)
            .await
            .map_err(|_| "Failed to create expansion entries")?;

        let mut s = self.shutdown_rx.clone();
        loop {
            tokio::select! {
                _ = s.changed() => {
                    println!("Killing scraper");
                    break
                }
                x = self.thing(&expansion) => {
                    if x.is_err() {
                        println!("Something went wrong scraping");
                        break;
                    }
                    println!("Now sleeping");
                }
            };

            tokio::select! {
                _ = s.changed() => {
                    println!("Killing scraper");
                    break
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(20)) => {
                    println!("Sleep completed");
                }
            }
        }

        Ok(())
    }

    async fn thing(&self, expansion: &Expansion) -> Result<(), String> {
        let cards = expansion.cards.iter().take(1).collect::<Vec<_>>();

        for card in cards {
            let last_listing_date = sqlx::query_as::<_,  (chrono::NaiveDate, )>("SELECT date FROM listings WHERE card_number = ? AND card_class = ? ORDER BY date DESC LIMIT 1")
                    .bind(card.number as u32)
                    .bind(card.class.to_string())
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(|_| "Failed to get last listing date")?
                    .map(|x| x.0);

            let final_listings = self
                .scrape_listings_for_card(card, expansion, last_listing_date)
                .await
                .map_err(|_| "Failed to scrape card")?;

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
                        .bind(std::convert::Into::<u64>::into(&x.price) as u32)
                        .bind(x.link.clone())
                        .bind(x.buying_format.get_bids().map(|x| x as u32))
                        .bind(x.buying_format.get_accepts_offers())
                        .bind(x.buying_format.get_offer_was_accepted())
                        .bind(card.number as u32)
                        .bind(card.class.to_string())
                })
                .execute(&self.pool)
                .await
                .map_err(|_| "Failed to update DB")?;
        }

        Ok(())
    }

    async fn scrape_listings_for_card<'a>(
        &self,
        card: &Pokemon,
        expansion: &Expansion,
        last_listing_date: Option<chrono::NaiveDate>,
    ) -> Result<Vec<Listing<'a>>, Box<dyn std::error::Error>> {
        let driver = &self.driver;
        // TODO: Consider clearing the text box
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
                thirtyfour::error::WebDriverErrorInner::NoSuchElement(_) => Ok(None),
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
        loop {
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
                    return Ok(final_listings);
                }

                let title = listing
                    .find(By::Css("a.s-item__link span[role=heading]"))
                    .await?
                    .text()
                    .await?;

                if !title.to_lowercase().contains(&card.name.to_lowercase())
                    && !card
                        .name
                        .to_lowercase()
                        .split_whitespace()
                        .all(|x| title.to_lowercase().contains(x))
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

                let Ok(price) = Money::from_str(price.as_str(), GBP) else {
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
                    thirtyfour::error::WebDriverErrorInner::NoSuchElement(_) => break,
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

        Ok(final_listings)
    }
}
