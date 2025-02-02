CREATE TABLE cards_new (
	set_name TEXT NOT NULL,
	expansion DECIMAL NOT NULL,
	number INTEGER NOT NULL,
	class TEXT NOT NULL,
	name TEXT NOT NULL,
	rarity TEXT NOT NULL,
	PRIMARY KEY (set_name, expansion, number, class)
);

INSERT INTO cards_new (set_name, expansion, number, class, name, rarity)
SELECT
	"Scarlet & Violet",
	8,
	number,
	class,
	name,
	rarity
FROM cards;

ALTER TABLE cards RENAME TO cards_backup; 
ALTER TABLE cards_new RENAME TO cards;

CREATE TABLE listings_new (
	id INTEGER NOT NULL,
	title TEXT NOT NULL,
	date TEXT NOT NULL,
	price INTEGER NOT NULL,
	link TEXT NOT NULL,
	bids INTEGER,
	accepts_offers BOOLEAN,
	offer_was_accepted BOOLEAN NOT NULL,
	card_set_name TEXT NOT NULL,
	card_expansion DECIMAL NOT NULL,
	card_number INTEGER NOT NULL,
	card_class TEXT NOT NULL,
	PRIMARY KEY (id, card_set_name, card_expansion, card_number, card_class),
	FOREIGN KEY (card_set_name, card_expansion, card_number, card_class) 
		REFERENCES cards(set_name, expansion, number, class) 
		ON DELETE RESTRICT
		ON UPDATE RESTRICT
);

INSERT INTO listings_new (
	id,
	title,
	date,
	price,
	link,
	bids,
	accepts_offers,
	offer_was_accepted,
	card_set_name,
	card_expansion,
	card_number,
	card_class
)
SELECT
	id,
	title,
	date,
	price,
	link,
	bids,
	accepts_offers,
	offer_was_accepted,
	"Scarlet & Violet",
	8,
	card_number,
	card_class
FROM listings;

ALTER TABLE listings RENAME TO listings_backup; 
ALTER TABLE listings_new RENAME TO listings;
