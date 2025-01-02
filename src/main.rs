#![warn(missing_debug_implementations, rust_2018_idioms, rustdoc::all)]
use chrono::{Datelike, NaiveDate};
use serde::Deserialize;
use thirtyfour::*;

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

#[derive(Debug)]
#[allow(dead_code)]
struct Listing {
    title: String,
    date: NaiveDate,
    price: String,
    link: String,
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

    let mut caps = DesiredCapabilities::chrome();
    caps.add_arg("--start-maximized")?;
    let driver = WebDriver::new("http://localhost:9515", caps).await?;

    //let cards = expansion.cards;
    //let cards = expansion.cards.into_iter().filter(|x| x.number == 238);
    //let cards = expansion.cards.into_iter().filter(|x| x.number == 1);
    let cards = expansion.cards.into_iter().take(1);
    //let cards = expansion.cards.into_iter().filter(|x| x.number == 4);
    //let cards = expansion
    //    .cards
    //    .into_iter()
    //    .filter(|x| x.number > expansion.expansion_total)
    //    .take(5);

    for card in cards {
        // TODO: Consider clearing the textbox
        driver.goto("https://ebay.co.uk").await?;

        println!("{card:#?}");

        let search_input = driver.find(By::Id("gh-ac")).await?;
        search_input
            .send_keys(format!(
                "{} {:0>3}/{}",
                card.name, card.number, expansion.expansion_total
            ))
            .await?;

        let search_btn = match driver.find(By::Id("gh-btn")).await {
            btn @ Ok(_) => btn,
            Err(_) => match driver.find(By::Id("gh-search-btn")).await {
                btn @ Ok(_) => btn,
                err => err,
            },
        }?;
        search_btn.click().await?;

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

        let sold_btn = driver
            .find(By::Css("input[type=checkbox][aria-label='Sold items']"))
            .await?;
        sold_btn.click().await?;

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

                let title = listing
                    .find(By::Css("a.s-item__link span[role=heading]"))
                    .await?
                    .text()
                    .await?;

                if !title.to_lowercase().contains(&card.name.to_lowercase()) {
                    println!("Title \"{}\" doesn't contain card name. Skipping.", title);
                    continue;
                }

                if match card.class {
                    Class::Regular => ["reverse holo", "reverse"]
                        .into_iter()
                        .any(|x| title.to_lowercase().contains(x)),
                    Class::ReverseHolo => title.to_lowercase().contains("regular"),
                    _ => false,
                } {
                    println!("Title \"{}\" contains blacklisted words. Skipping.", title);
                    continue;
                }

                let date = listing
                    .find(By::Css(".s-item__caption"))
                    .await?
                    .text()
                    .await?;

                let date =
                    NaiveDate::parse_from_str(date.trim_start_matches("Sold "), "%-d %b %Y")?;

                let price = listing
                    .find(By::Css(".s-item__price"))
                    .await?
                    .text()
                    .await?;

                let link = listing
                    .find(By::Css(".s-item__link"))
                    .await?
                    .prop("href")
                    .await?
                    .unwrap_or("".into());

                let listing = Listing {
                    title,
                    date,
                    price,
                    link,
                };

                final_listings.push(listing);
            }

            let next_page_btn = match driver.find(By::Css("a.pagination__next")).await {
                btn @ Ok(_) => btn,
                Err(err) => match err.as_inner() {
                    error::WebDriverErrorInner::NoSuchElement(_) => break,
                    _ => Err(err),
                },
            }?;
            next_page_btn.click().await?;

            page_count += 1;
            if page_count > 100 {
                println!("PAGINATION LIMIT");
                break;
            }
        }

        dbg!(final_listings);

        std::thread::sleep(std::time::Duration::new(10, 0));
    }

    Ok(())
}
