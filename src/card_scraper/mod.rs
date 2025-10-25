use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::Sqlite;
use thirtyfour::{By, Capabilities, WebDriver};

use crate::currency::{Money, GBP};

const PAGINATION_LIMIT: usize = 100;

#[derive(Debug, Deserialize, Clone)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    DoubleRare,
    AceSpecRare,
    IllustrationRare,
    UltraRare,
    SpecialIllustrationRare,
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
    Regular,
    #[serde(rename = "Parallel")]
    ReverseHolo,
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
    #[serde(rename = "variants")]
    pub class: Vec<Class>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct Expansion {
    pub set_name: String,
    pub expansion_name: String,
    pub expansion_number: f32,
    pub expansion_total: usize,
    pub cards: Vec<Pokemon>,
}

pub struct CardScaper {
    pool: sqlx::Pool<Sqlite>,
    shutdown_rx: std::sync::Arc<tokio::sync::Notify>,
    sleep_seconds: u64,
    web_driver_url: String,
    web_driver_capabilities: Capabilities,
}

impl CardScaper {
    pub fn new(
        pool: sqlx::Pool<Sqlite>,
        web_driver_url: impl Into<String>,
        web_driver_capabilities: impl Into<Capabilities>,
        shutdown_rx: std::sync::Arc<tokio::sync::Notify>,
        sleep_seconds: u64,
    ) -> Self {
        Self {
            pool,
            shutdown_rx,
            sleep_seconds,
            web_driver_url: web_driver_url.into(),
            web_driver_capabilities: web_driver_capabilities.into(),
        }
    }

    pub async fn start_scraping_expansions(
        &self,
        expansions: Vec<Expansion>,
    ) -> Result<(), String> {
        let (ei, ci) = sqlx::query_as::<_, (String, f32, u32)>(
            "SELECT set_name, CAST(expansion AS REAL) AS expansion, number FROM scraper_progress WHERE id = 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Failed to get scraper progress: {e}"))?
        .and_then(|(set_name, expansion, number)| {
            expansions
                .iter()
                .position(|e| e.set_name == set_name && e.expansion_number == expansion)
                .map(|i| {
                    (i, expansions[i].cards.iter().position(|c| c.number == number as usize).unwrap_or_default())
                })
        })
        .unwrap_or_default();

        for expansion in &expansions {
            expansion
                .cards
                .iter()
                .fold(
                    sqlx::query(&format!(
                        "INSERT INTO cards (set_name, expansion, number, class, name, rarity) VALUES {} ON CONFLICT DO NOTHING",
                        expansion
                            .cards
                            .iter()
                            .flat_map(|card| card.class.iter().map(|_|  "(?,?,?,?,?,?)"))
                            .collect::<Vec<_>>()
                            .join(",")
                    )),
                    |acc, x| {
                        x
                            .class
                            .iter()
                            .fold(acc, |acc, c| {
                                acc
                                    .bind(expansion.set_name.clone())
                                    .bind(expansion.expansion_number)
                                    .bind(x.number as u32)
                                    .bind(c.to_string())
                                    .bind(x.name.clone())
                                    .bind(x.rarity.to_string())
                            }
                        )
                    },
                )
                .execute(&self.pool)
                .await
                .map_err(|e| format!("Failed to create expansion entries: {e}"))?;
        }

        loop {
            let driver = WebDriver::new(
                self.web_driver_url.clone(),
                self.web_driver_capabilities.clone(),
            )
            .await
            .map_err(|e| e.to_string())?;

            for expansion in &expansions[ei..] {
                tokio::select! {
                    _ = self.shutdown_rx.notified() => {
                        println!("Killing scraper");
                        return Ok(());
                    }
                    x = self.scrape_expansion(expansion, ci, &driver) => {
                        if let Err(a) = x {
                            println!("Something went wrong scraping: {a:?}");

                            let timestamp = format!("screenshots/{}.png", chrono::Utc::now().to_rfc3339());
                            if let Err(e) = driver.screenshot(std::path::Path::new(&timestamp)).await {
                                println!("Failed to take screenshot {e:?}");
                            }

                            return Err(a);
                        }
                        println!("Now sleeping");
                    }
                };
            }

            drop(driver);

            sqlx::query("DELETE FROM scraper_progress")
                .execute(&self.pool)
                .await
                .map_err(|e| format!("Failed to delete scraper progress: {e}"))?;

            tokio::select! {
                _ = self.shutdown_rx.notified() => {
                    println!("Killing scraper");
                    return Ok(());
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(self.sleep_seconds)) => {
                    println!("Sleep completed");
                }
            }
        }
    }

    async fn scrape_expansion(
        &self,
        expansion: &Expansion,
        card_start: usize,
        driver: &WebDriver,
    ) -> Result<(), String> {
        let cards = &expansion.cards[card_start..]
            .iter()
            .flat_map(|card| {
                card.class.iter().map(|class| Pokemon {
                    name: card.name.clone(),
                    number: card.number,
                    rarity: card.rarity.clone(),
                    class: vec![class.clone()],
                })
            })
            .collect::<Vec<_>>();

        for card in cards {
            let last_listing_date = sqlx::query_as::<_, (chrono::NaiveDate,)>(
                "
                    SELECT date
                    FROM listings
                    JOIN listings_cards
                      ON listings_cards.listing_id = listings.id
                    WHERE listings_cards.card_set_name = ?
                      AND listings_cards.card_expansion = ?
                      AND listings_cards.card_number = ?
                      AND listings_cards.card_class = ?
                    ORDER BY date DESC
                    LIMIT 1
                    ",
            )
            .bind(expansion.set_name.clone())
            .bind(expansion.expansion_number)
            .bind(card.number as u32)
            .bind(card.class.first().unwrap().to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| format!("Failed to last listing date: {e}"))?
            .map(|x| x.0);

            let final_listings = self
                .scrape_listings_for_card(card, expansion, last_listing_date, driver)
                .await
                .map_err(|e| format!("Failed to scrape card: {e:?}"))?;

            let mut txn = self
                .pool
                .begin()
                .await
                .map_err(|e| format!("Error creating transaction: {e}"))?;

            let r: Result<(), sqlx::error::Error> = async {
                if !final_listings.is_empty() {
                    final_listings
                        .iter()
                        .fold(
                            sqlx::query(&format!(
                                "
                                INSERT INTO listings
                                    (id, title, date, price, link, bids, accepts_offers, offer_was_accepted) 
                                VALUES {} 
                                ON CONFLICT DO NOTHING
                                ",
                                final_listings
                                    .iter()
                                    .map(|_| "(?,?,?,?,?,?,?,?)")
                                    .collect::<Vec<_>>()
                                    .join(",")
                            )),
                            |acc, x| {
                                acc.bind(x.id as u32)
                                    .bind(x.title.clone())
                                    .bind(x.date)
                                    .bind(std::convert::Into::<u64>::into(&x.price) as u32)
                                    .bind(x.link.clone())
                                    .bind(x.buying_format.get_bids().map(|x| x as u32))
                                    .bind(x.buying_format.get_accepts_offers())
                                    .bind(x.buying_format.get_offer_was_accepted())
                            },
                        )
                        .execute(&mut *txn)
                        .await?;

                    final_listings
                        .iter()
                        .fold(
                            sqlx::query(&format!(
                                "
                                INSERT INTO listings_cards
                                    (listing_id, card_set_name, card_expansion, card_number, card_class) 
                                VALUES {} 
                                ON CONFLICT DO NOTHING
                                ",
                                final_listings
                                    .iter()
                                    .map(|_| "(?,?,?,?,?)")
                                    .collect::<Vec<_>>()
                                    .join(",")
                            )),
                            |acc, x| acc
                                .bind(x.id as u32)
                                .bind(expansion.set_name.clone())
                                .bind(expansion.expansion_number )
                                .bind(card.number as u32)
                                .bind(card.class.first().unwrap().to_string()),
                        )
                        .execute(&mut *txn)
                        .await?;

                }

                sqlx::query(
                    "
                    INSERT OR REPLACE INTO scraper_progress 
                        (id, set_name, expansion, number, class)
                    VALUES
                        (1, ?, ?, ?, ?)"
                )
                    .bind(expansion.set_name.clone())
                    .bind(expansion.expansion_number )
                    .bind(card.number as u32)
                    .bind(card.class.first().unwrap().to_string())
                    .execute(&mut *txn)
                    .await?;

                Ok(())
            }.await;

            match r {
                err @ Err(_) => {
                    if let Err(e2) = txn.rollback().await {
                        eprintln!("Failed to rollback transaction with Error: {e2}");
                    }
                    err
                }
                Ok(_) => txn.commit().await,
            }
            .map_err(|e| format!("Failed to create listing: {e}"))?;
        }

        Ok(())
    }

    async fn scrape_listings_for_card<'a>(
        &self,
        card: &Pokemon,
        expansion: &Expansion,
        last_listing_date: Option<chrono::NaiveDate>,
        driver: &WebDriver,
    ) -> Result<Vec<Listing<'a>>, Box<dyn std::error::Error>> {
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

        // INFO: The page takes a while to load so we add retry logic to the first find
        for i in 0..5 {
            match driver
                .find(By::Css("input[type=checkbox][aria-label='Sold items']"))
                .await
            {
                Ok(checkbox) => {
                    checkbox.click().await?;
                    break;
                }
                e @ Err(_) => {
                    if i >= 4 {
                        e?;
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                }
            }
        }

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

                if !class_names.split_whitespace().any(|x| x == "s-card") {
                    if listing.text().await? == "Results matching fewer words" {
                        println!("Reached end of good results");
                        break;
                    };

                    continue;
                }

                let date = NaiveDate::parse_from_str(
                    listing
                        .find(By::Css(".s-card__caption"))
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
                    .find(By::Css("a > div.s-card__title span"))
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

                if match card.class.first().unwrap() {
                    Class::Regular => ["reverse holo", "reverse"]
                        .into_iter()
                        .any(|x| title.to_lowercase().contains(x)),
                    Class::ReverseHolo => title.to_lowercase().contains("regular"),
                    Class::Foil => false,
                } {
                    println!("Title \"{}\" contains blacklisted words. Skipping.", title);
                    continue;
                }

                if match card.class.first().unwrap() {
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
                    .find(By::Css(".s-card__price"))
                    .await?
                    .text()
                    .await?;

                let Ok(price) = Money::from_str(price.as_str(), GBP) else {
                    println!("Failed to parse price {price}. Skipping.");
                    continue;
                };

                let link = listing
                    .find(By::Css(".su-card-container__header a"))
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

                let buying_format = match listing
                    .find(By::Css(".su-card-container__attributes__primary .s-card__attribute-row:nth-child(2)"))
                    .await?
                    .text()
                    .await?
                    .as_str()
                {
                    "Buy It Now" => BuyingFormat::BuyItNow {
                        accepts_offers: false,
                        offer_was_accepted: false,
                    },
                    "or Best Offer" => BuyingFormat::BuyItNow {
                        accepts_offers: true,
                        offer_was_accepted: false,
                    },
                    "Best Offer accepted" => BuyingFormat::BuyItNow {
                        accepts_offers: true,
                        offer_was_accepted: true,
                    },
                    bids => {
                        BuyingFormat::Auction {
                            bids: bids
                                .split_whitespace()
                                .next()
                                .expect("should always have a value")
                                .parse()
                                .expect("should always be a number"),
                            offer_was_accepted: listing
                                .find(By::Css(".su-card-container__attributes__primary .s-card__attribute-row:nth-child(3)"))
                                .await?.text().await? == "Best Offer accepted"
                        }
                    }
                };

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

trait Finder {
    async fn find(&self, by: By) -> thirtyfour::error::WebDriverResult<thirtyfour::WebElement>;
    async fn find_all(
        &self,
        by: By,
    ) -> thirtyfour::error::WebDriverResult<Vec<thirtyfour::WebElement>>;
}

impl Finder for std::sync::Arc<thirtyfour::session::handle::SessionHandle> {
    async fn find(&self, by: By) -> thirtyfour::error::WebDriverResult<thirtyfour::WebElement> {
        self.find(by).await
    }

    async fn find_all(
        &self,
        by: By,
    ) -> thirtyfour::error::WebDriverResult<Vec<thirtyfour::WebElement>> {
        self.find_all(by).await
    }
}

impl Finder for thirtyfour::WebElement {
    async fn find(&self, by: By) -> thirtyfour::error::WebDriverResult<thirtyfour::WebElement> {
        self.find(by).await
    }

    async fn find_all(
        &self,
        by: By,
    ) -> thirtyfour::error::WebDriverResult<Vec<thirtyfour::WebElement>> {
        self.find_all(by).await
    }
}

trait TryFind {
    async fn try_find(&self, by: By) -> thirtyfour::error::WebDriverResult<thirtyfour::WebElement>;
    async fn try_find_all(
        &self,
        by: By,
    ) -> thirtyfour::error::WebDriverResult<Vec<thirtyfour::WebElement>>;
}

impl<T> TryFind for T
where
    T: Finder,
{
    async fn try_find(&self, by: By) -> thirtyfour::error::WebDriverResult<thirtyfour::WebElement> {
        const MAX_RETRIES: usize = 4;
        const RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(3);

        for i in 0..MAX_RETRIES {
            dbg!(i, &by);
            if let Ok(element) = self.find(by.clone()).await {
                return Ok(element);
            }
            tokio::time::sleep(RETRY_DELAY).await;
        }

        self.find(by).await
    }

    async fn try_find_all(
        &self,
        by: By,
    ) -> thirtyfour::error::WebDriverResult<Vec<thirtyfour::WebElement>> {
        const MAX_RETRIES: usize = 4;
        const RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(3);

        for _ in 0..MAX_RETRIES {
            if let Ok(element) = self.find_all(by.clone()).await {
                return Ok(element);
            }
            tokio::time::sleep(RETRY_DELAY).await;
        }

        self.find_all(by).await
    }
}
