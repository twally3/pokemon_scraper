CREATE TABLE listings_new (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    date TEXT NOT NULL,
    price INTEGER NOT NULL,
    link TEXT NOT NULL,
    bids INTEGER,
    accepts_offers BOOLEAN,
    offer_was_accepted BOOLEAN NOT NULL
);

INSERT INTO listings_new (id, title, date, price, link, bids, accepts_offers, offer_was_accepted)
SELECT 
    id,
    MIN(title) as title,
    MIN(date) as date, 
    MIN(price) as price,
    MIN(link) as link,
    MIN(bids) as bids,
    MIN(accepts_offers) as accepts_offers,
    MIN(offer_was_accepted) as offer_was_accepted
FROM listings
GROUP BY id;

ALTER TABLE listings RENAME TO listings_backup_2; 
ALTER TABLE listings_new RENAME TO listings; 

CREATE TABLE listings_cards (
	listing_id INTEGER NOT NULL,
	card_set_name TEXT NOT NULL,
	card_expansion DECIMAL NOT NULL,
	card_number INTEGER NOT NULL,
	card_class TEXT NOT NULL,
	PRIMARY KEY (listing_id, card_set_name, card_expansion, card_number, card_class),
	FOREIGN KEY (listing_id) 
		REFERENCES listings(id)
		ON DELETE RESTRICT
		ON UPDATE RESTRICT,
	FOREIGN KEY (card_set_name, card_expansion, card_number, card_class)
		REFERENCES cards(set_name, expansion, number, class)
		ON DELETE RESTRICT
		ON UPDATE RESTRICT
);

INSERT INTO listings_cards (listing_id, card_set_name, card_expansion, card_number, card_class)
SELECT DISTINCT 
	id as listing_id,
	card_set_name,
	card_expansion,
	card_number,
	card_class
FROM listings_backup_2;
