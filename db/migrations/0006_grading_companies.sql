CREATE TABLE grading_companies (
	id INTEGER PRIMARY KEY,
	initials TEXT NOT NUlL
);

ALTER TABLE listings
ADD COLUMN graded_by DEFAULT NULL REFERENCES grading_companies(id);

INSERT INTO grading_companies
	(id, initials)
VALUES
	(1, "PSA"),
	(2, "ACE"),
	(3, "CGC"),
	(4, "UGC"),
	(5, "BGS"),
	(6, "SGC"),
	(7, "GMA");

UPDATE listings
SET graded_by = (
    SELECT id
    FROM grading_companies gc
    WHERE listings.title LIKE '% ' || gc.initials || '%'
    ORDER BY id
    LIMIT 1
)
WHERE EXISTS (
    SELECT 1
    FROM grading_companies gc
    WHERE listings.title LIKE '% ' || gc.initials || '%'
);
