CREATE TABLE scraper_progress (
	id INTEGER PRIMARY KEY CHECK (id = 1),
	set_name TEXT NOT NULL,
	expansion DECIMAL NOT NULL,
	number INTEGER NOT NULL,
	class TEXT NOT NULL
);
