CREATE VIEW ranked_listings AS
SELECT
	listings.*,
	cards.set_name AS card_set_name,
	cards.expansion AS card_expansion,
	cards.number AS card_number,
	cards.class AS card_class,
	cards.name AS card_name,
	cards.rarity AS card_rarity,
	ROW_NUMBER() OVER (
		PARTITION BY
			cards.set_name,
			cards.expansion,
			cards.number,
			cards.class
		ORDER BY
			listings.date DESC
	) AS listing_rank
FROM
	cards
	LEFT JOIN listings_cards ON listings_cards.card_set_name = cards.set_name
	AND listings_cards.card_expansion = cards.expansion
	AND listings_cards.card_number = cards.number
	AND listings_cards.card_class = cards.class
	LEFT JOIN listings ON listings.id = listings_cards.listing_id;
