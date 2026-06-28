PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS books (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    author TEXT NOT NULL,
    genre TEXT NOT NULL,
    year INTEGER NOT NULL,
    isbn TEXT NOT NULL,
    cover_color TEXT NOT NULL,
    aspect_ratio REAL NOT NULL DEFAULT 0.68,
    tags TEXT NOT NULL DEFAULT '',
    is_new_arrival INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS book_copies (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    book_id TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    condition TEXT NOT NULL CHECK (condition IN ('New (Sealed)', 'New (Open)', 'Fine', 'Like New', 'Very Good', 'Good', 'Fair', 'Poor')),
    price REAL NOT NULL CHECK (price >= 0),
    notes TEXT,
    format TEXT NOT NULL CHECK (format IN ('Hardcover', 'Paperback', 'Trade Paperback')),
    stock INTEGER NOT NULL DEFAULT 1 CHECK (stock >= 0),
    is_sold INTEGER NOT NULL DEFAULT 0,
    is_staff_pick INTEGER NOT NULL DEFAULT 0,
    staff_quote TEXT,
    seal_style TEXT NOT NULL DEFAULT 'none',
    seal_text TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (book_id, condition, format, price)
);

CREATE INDEX IF NOT EXISTS idx_books_genre ON books(genre);
CREATE INDEX IF NOT EXISTS idx_books_title_author ON books(title, author);
CREATE INDEX IF NOT EXISTS idx_book_copies_book_id ON book_copies(book_id);
CREATE INDEX IF NOT EXISTS idx_book_copies_available ON book_copies(is_sold, price);

INSERT OR IGNORE INTO books (id, title, author, genre, year, isbn, cover_color, aspect_ratio, tags, is_new_arrival) VALUES
('b001', 'The Night Circus', 'Erin Morgenstern', 'Fiction', 2012, '9780307744432', '#1a2e26', 0.65, 'magical,atmospheric,romance', 0),
('b002', 'Kitchen Confidential', 'Anthony Bourdain', 'Memoir', 2007, '9780060899226', '#6b1827', 0.72, 'food,memoir,sharp', 1),
('b003', 'Dune', 'Frank Herbert', 'Science Fiction', 2019, '9780441172719', '#a7482f', 0.68, 'classic,desert,politics', 1),
('b004', 'The Secret History', 'Donna Tartt', 'Mystery', 2004, '9781400031702', '#233e54', 0.75, 'campus,literary,dark', 0),
('b005', 'Braiding Sweetgrass', 'Robin Wall Kimmerer', 'Nature', 2020, '9781571313560', '#284234', 0.70, 'ecology,essays,indigenous', 0),
('b006', 'The Thursday Murder Club', 'Richard Osman', 'Mystery', 2021, '9781984880987', '#b48c34', 0.62, 'cozy,witty,series', 1),
('b007', 'A Gentleman in Moscow', 'Amor Towles', 'Historical Fiction', 2019, '9780143110439', '#263e52', 0.74, 'elegant,hotel,historical', 0),
('b008', 'The Hobbit', 'J.R.R. Tolkien', 'Fantasy', 2001, '9780618260300', '#4e5933', 0.60, 'classic,adventure,quest', 0),
('b009', 'Salt Fat Acid Heat', 'Samin Nosrat', 'Cookbooks', 2017, '9781476753836', '#ab5433', 0.82, 'cooking,illustrated,technique', 1),
('b010', 'The Warmth of Other Suns', 'Isabel Wilkerson', 'History', 2011, '9780679763888', '#543928', 0.71, 'history,migration,america', 0),
('b011', 'Project Hail Mary', 'Andy Weir', 'Science Fiction', 2022, '9780593135228', '#214e63', 0.66, 'space,clever,survival', 1),
('b012', 'The Vanishing Half', 'Brit Bennett', 'Fiction', 2021, '9780525536963', '#73334c', 0.73, 'family,identity,literary', 0),
('b013', 'World of Wonders', 'Aimee Nezhukumatathil', 'Nature', 2020, '9781571313652', '#2b5c5a', 0.78, 'essays,nature,lyrical', 0),
('b014', 'The Book Thief', 'Markus Zusak', 'Historical Fiction', 2007, '9780375842207', '#3f4b54', 0.63, 'historical,young adult,moving', 0),
('b015', 'The Midnight Library', 'Matt Haig', 'Fiction', 2020, '9780525559474', '#204663', 0.77, 'reflective,second chances,bookish', 1),
('b016', 'The Overstory', 'Richard Powers', 'Fiction', 2019, '9780393356687', '#2b522b', 0.69, 'trees,pulitzer,ambitious', 0),
('b017', 'Educated', 'Tara Westover', 'Memoir', 2020, '9780399590528', '#5c3d2e', 0.64, 'memoir,resilience,family', 0),
('b018', 'The Art Forger', 'B.A. Shapiro', 'Mystery', 2013, '9781616203160', '#523b6b', 0.73, 'art,heist,boston', 0),
('b019', 'Pachinko', 'Min Jin Lee', 'Historical Fiction', 2017, '9781455563920', '#7d2b24', 0.67, 'family saga,korea,japan', 1),
('b020', 'The Splendid and the Vile', 'Erik Larson', 'History', 2020, '9780385348713', '#182d42', 0.79, 'churchill,wwii,narrative', 0),
('b021', 'Circe', 'Madeline Miller', 'Fantasy', 2019, '9780316556323', '#8c4f1c', 0.66, 'mythology,greek,literary', 0),
('b022', 'The Body Keeps the Score', 'Bessel van der Kolk', 'Psychology', 2015, '9780143127741', '#2d444f', 0.72, 'psychology,trauma,health', 0),
('b023', 'The Joy of Cooking', 'Irma S. Rombauer', 'Cookbooks', 2006, '9780743246262', '#6b401d', 0.84, 'classic,reference,cooking', 0),
('b024', 'The House in the Cerulean Sea', 'TJ Klune', 'Fantasy', 2021, '9781250217318', '#235c70', 0.69, 'warm,found family,hopeful', 1),
('b025', 'Say Nothing', 'Patrick Radden Keefe', 'History', 2020, '9780307279286', '#323528', 0.70, 'ireland,true crime,politics', 0),
('b026', 'The Invisible Life of Addie LaRue', 'V.E. Schwab', 'Fantasy', 2020, '9780765387561', '#13273b', 0.78, 'immortality,romance,paris', 1),
('b027', 'Atomic Habits', 'James Clear', 'Business', 2018, '9780735211292', '#162329', 0.65, 'habits,productivity,practical', 0),
('b028', 'The Feather Thief', 'Kirk Wallace Johnson', 'Mystery', 2019, '9781101981634', '#2e4a46', 0.71, 'true crime,natural history,odd', 0);

INSERT OR IGNORE INTO book_copies (book_id, condition, price, notes, format, stock, is_staff_pick, staff_quote, seal_style, seal_text) VALUES
('b001', 'Very Good', 9.50, 'Clean pages, faint spine crease, soft corner wear.', 'Trade Paperback', 3, 1, 'An atmospheric, dream-like escape. It feels like stepping into a midnight festival. - Sarah', 'circle', 'DB'),
('b002', 'Good', 8.75, 'Readable copy with shelf scuffs and a small owner stamp.', 'Paperback', 2, 0, 'A wonderful pre-loved copy, carefully chosen for our shelf. - Davis Team', 'square', 'USED'),
('b003', 'Very Good', 11.25, 'Tight binding, light page tanning, no markings.', 'Paperback', 4, 1, 'The definitive epic. The worldbuilding is so rich you will feel the sand on your fingers. - Dan', 'diamond', '1st'),
('b004', 'Good', 10.50, 'Some margin notes in pencil, square spine.', 'Trade Paperback', 2, 1, 'A dark campus mystery that is impossible to put down. Truly haunting. - Claire', 'none', ''),
('b005', 'Like New', 12.75, 'Gift-quality copy with a tiny back-cover nick.', 'Paperback', 3, 1, 'A wonderful pre-loved copy, carefully chosen for our shelf. - Davis Team', 'circle', 'CRISP'),
('b006', 'Very Good', 7.95, 'Bright cover, minor shelf rubbing.', 'Paperback', 5, 0, 'A wonderful pre-loved copy, carefully chosen for our shelf. - Davis Team', 'diamond', 'GOLD'),
('b007', 'Very Good', 9.25, 'Smooth spine, lightly bumped lower corner.', 'Trade Paperback', 3, 1, 'A wonderful pre-loved copy, carefully chosen for our shelf. - Davis Team', 'square', 'DB'),
('b008', 'Fair', 6.95, 'Well-loved reading copy with creases and tanned pages.', 'Paperback', 6, 0, 'A wonderful pre-loved copy, carefully chosen for our shelf. - Davis Team', 'none', ''),
('b009', 'Very Good', 18.50, 'No food stains, jacket has a short closed tear.', 'Hardcover', 1, 1, 'A wonderful pre-loved copy, carefully chosen for our shelf. - Davis Team', 'circle', 'COOK'),
('b010', 'Good', 13.95, 'Strong reading copy with several dog-eared pages.', 'Paperback', 2, 0, 'A wonderful pre-loved copy, carefully chosen for our shelf. - Davis Team', 'none', ''),
('b011', 'Like New', 10.95, 'Crisp copy with a clean unbroken spine.', 'Paperback', 4, 0, 'A wonderful pre-loved copy, carefully chosen for our shelf. - Davis Team', 'circle', 'NEW'),
('b012', 'Very Good', 8.95, 'Clean text block, faint bend on front cover.', 'Paperback', 3, 0, 'A wonderful pre-loved copy, carefully chosen for our shelf. - Davis Team', 'diamond', 'DB'),
('b013', 'Very Good', 7.50, 'Small remainder mark, otherwise fresh.', 'Hardcover', 2, 1, 'A wonderful pre-loved copy, carefully chosen for our shelf. - Davis Team', 'square', 'WILD'),
('b014', 'Good', 6.75, 'Moderate cover wear, clean pages.', 'Paperback', 5, 0, 'A wonderful pre-loved copy, carefully chosen for our shelf. - Davis Team', 'none', ''),
('b015', 'Like New', 8.25, 'Jacket and boards look nearly untouched.', 'Hardcover', 2, 0, 'A wonderful pre-loved copy, carefully chosen for our shelf. - Davis Team', 'circle', 'SOUL'),
('b016', 'Good', 9.75, 'One cracked spine line, pages bright and unmarked.', 'Paperback', 3, 1, 'A wonderful pre-loved copy, carefully chosen for our shelf. - Davis Team', 'circle', 'PUL'),
('b017', 'Very Good', 7.75, 'Light edge wear, no notes or highlighting.', 'Paperback', 4, 0, 'A wonderful pre-loved copy, carefully chosen for our shelf. - Davis Team', 'none', ''),
('b018', 'Good', 6.50, 'Clean copy with a remainder dot.', 'Trade Paperback', 2, 0, 'A wonderful pre-loved copy, carefully chosen for our shelf. - Davis Team', 'diamond', 'ART'),
('b019', 'Very Good', 10.25, 'Minor shelf wear, tight and clean inside.', 'Paperback', 3, 1, 'A wonderful pre-loved copy, carefully chosen for our shelf. - Davis Team', 'circle', 'DB'),
('b020', 'Very Good', 12.50, 'Jacket has light rubbing, pages pristine.', 'Hardcover', 2, 0, 'A wonderful pre-loved copy, carefully chosen for our shelf. - Davis Team', 'square', 'WAR'),
('b021', 'Very Good', 8.95, 'Fresh pages, small crease on back cover.', 'Paperback', 4, 1, 'A wonderful pre-loved copy, carefully chosen for our shelf. - Davis Team', 'diamond', 'MYTH'),
('b022', 'Good', 11.50, 'A few highlighted passages in early chapters.', 'Paperback', 2, 0, 'A wonderful pre-loved copy, carefully chosen for our shelf. - Davis Team', 'none', ''),
('b023', 'Good', 16.95, 'Sturdy kitchen copy with light page waviness.', 'Hardcover', 1, 0, 'A wonderful pre-loved copy, carefully chosen for our shelf. - Davis Team', 'circle', 'JOY'),
('b024', 'Like New', 9.95, 'Excellent copy with a bright, clean cover.', 'Paperback', 3, 1, 'A wonderful pre-loved copy, carefully chosen for our shelf. - Davis Team', 'circle', 'LOVE'),
('b025', 'Very Good', 9.50, 'Unmarked pages, faint corner curl.', 'Paperback', 2, 0, 'A wonderful pre-loved copy, carefully chosen for our shelf. - Davis Team', 'diamond', 'TRUE'),
('b026', 'Very Good', 10.75, 'Jacket lightly rubbed, boards and pages clean.', 'Hardcover', 2, 0, 'A wonderful pre-loved copy, carefully chosen for our shelf. - Davis Team', 'square', 'MAGIC'),
('b027', 'Good', 8.50, 'A few underlined sections, clean cover.', 'Paperback', 5, 0, 'A wonderful pre-loved copy, carefully chosen for our shelf. - Davis Team', 'circle', 'FOCUS'),
('b028', 'Very Good', 7.25, 'Clean and square with light shelf scuffs.', 'Paperback', 3, 1, 'A wonderful pre-loved copy, carefully chosen for our shelf. - Davis Team', 'circle', 'FEATH');
