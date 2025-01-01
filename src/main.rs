#![warn(missing_debug_implementations, rust_2018_idioms, rustdoc::all)]
use serde::Deserialize;

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

#[derive(Debug, Deserialize)]
enum Class {
    #[serde(rename = "advanced-pkmn_regular-unchecked")]
    Regular,
    #[serde(rename = "advanced-pkmn_rev_holo-unchecked")]
    ReverseHolo,
    #[serde(rename = "advanced-pkmn_foil-unchecked")]
    Foil,
}

#[derive(Debug, Deserialize)]
struct Pokemon {
    name: String,
    number: usize,
    rarity: Rarity,
    class: Class,
}

#[derive(Debug, Deserialize)]
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

    dbg!(&expansion);

    Ok(())
}
