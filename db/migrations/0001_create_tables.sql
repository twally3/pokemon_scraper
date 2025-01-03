CREATE TABLE cards (
	number INTEGER NOT NULL,
	class TEXT NOT NULL,
	name TEXT NOT NULL,
	rarity TEXT NOT NULL,
	PRIMARY KEY (number, class)
);

CREATE TABLE listings (
	id INTEGER NOT NULL,
	title TEXT NOT NULL,
	date TEXT NOT NULL,
	price REAL NOT NULL,
	link TEXT NOT NULL,
	bids INTEGER,
	accepts_offers BOOLEAN,
	offer_was_accepted BOOLEAN NOT NULL,
	card_number INTEGER NOT NULL,
	card_class TEXT NOT NULL,
	PRIMARY KEY (id, card_number, card_class),
	FOREIGN KEY (card_number, card_class) 
		REFERENCES cards(number, class) 
		ON DELETE RESTRICT
		ON UPDATE RESTRICT
);
