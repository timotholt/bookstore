PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS authors (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    sort_name TEXT NOT NULL,
    biography TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS genres (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    parent_id INTEGER REFERENCES genres(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS books (
    id TEXT PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    primary_author_id INTEGER NOT NULL REFERENCES authors(id),
    primary_genre_id INTEGER NOT NULL REFERENCES genres(id),
    year INTEGER NOT NULL,
    isbn TEXT NOT NULL UNIQUE,
    publication_date TEXT,
    cover_color TEXT NOT NULL,
    aspect_ratio REAL NOT NULL DEFAULT 0.68,
    tags TEXT NOT NULL DEFAULT '',
    search_text TEXT NOT NULL DEFAULT '',
    is_new_arrival INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS book_authors (
    book_id TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    author_id INTEGER NOT NULL REFERENCES authors(id) ON DELETE CASCADE,
    role TEXT NOT NULL DEFAULT 'Author',
    position INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (book_id, author_id, role)
);

CREATE TABLE IF NOT EXISTS book_genres (
    book_id TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    genre_id INTEGER NOT NULL REFERENCES genres(id) ON DELETE CASCADE,
    is_primary INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (book_id, genre_id)
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
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (book_id, condition, format, price)
);

CREATE TABLE IF NOT EXISTS book_collections (
    slug TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    cache_key TEXT NOT NULL UNIQUE,
    cache_ttl_seconds INTEGER NOT NULL DEFAULT 300,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS book_collection_items (
    collection_slug TEXT NOT NULL REFERENCES book_collections(slug) ON DELETE CASCADE,
    book_id TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    position INTEGER NOT NULL DEFAULT 100,
    is_active INTEGER NOT NULL DEFAULT 1,
    starts_at TEXT,
    ends_at TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (collection_slug, book_id)
);

CREATE TABLE IF NOT EXISTS cache_tags (
    tag TEXT PRIMARY KEY,
    description TEXT NOT NULL DEFAULT '',
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS book_cache_tags (
    book_id TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    tag TEXT NOT NULL REFERENCES cache_tags(tag) ON DELETE CASCADE,
    PRIMARY KEY (book_id, tag)
);

CREATE INDEX IF NOT EXISTS idx_authors_slug ON authors(slug);
CREATE INDEX IF NOT EXISTS idx_genres_slug ON genres(slug);
CREATE INDEX IF NOT EXISTS idx_books_primary_author ON books(primary_author_id);
CREATE INDEX IF NOT EXISTS idx_books_primary_genre ON books(primary_genre_id);
CREATE INDEX IF NOT EXISTS idx_books_title ON books(title);
CREATE INDEX IF NOT EXISTS idx_books_new_arrival ON books(is_new_arrival, updated_at);
CREATE INDEX IF NOT EXISTS idx_books_search ON books(search_text);
CREATE INDEX IF NOT EXISTS idx_book_authors_author ON book_authors(author_id, position);
CREATE INDEX IF NOT EXISTS idx_book_genres_genre ON book_genres(genre_id, is_primary);
CREATE INDEX IF NOT EXISTS idx_book_copies_book_id ON book_copies(book_id);
CREATE INDEX IF NOT EXISTS idx_book_copies_available ON book_copies(is_sold, stock, price);
CREATE INDEX IF NOT EXISTS idx_book_copies_staff_pick ON book_copies(is_staff_pick, price);
CREATE INDEX IF NOT EXISTS idx_book_collection_items_lookup ON book_collection_items(collection_slug, is_active, position);
CREATE INDEX IF NOT EXISTS idx_book_cache_tags_tag ON book_cache_tags(tag);

INSERT OR IGNORE INTO authors (slug, name, sort_name) VALUES
('erin-morgenstern', 'Erin Morgenstern', 'Morgenstern, Erin'),
('anthony-bourdain', 'Anthony Bourdain', 'Bourdain, Anthony'),
('frank-herbert', 'Frank Herbert', 'Herbert, Frank'),
('donna-tartt', 'Donna Tartt', 'Tartt, Donna'),
('robin-wall-kimmerer', 'Robin Wall Kimmerer', 'Kimmerer, Robin Wall'),
('richard-osman', 'Richard Osman', 'Osman, Richard'),
('amor-towles', 'Amor Towles', 'Towles, Amor'),
('j-r-r-tolkien', 'J.R.R. Tolkien', 'Tolkien, J.R.R.'),
('samin-nosrat', 'Samin Nosrat', 'Nosrat, Samin'),
('isabel-wilkerson', 'Isabel Wilkerson', 'Wilkerson, Isabel'),
('andy-weir', 'Andy Weir', 'Weir, Andy'),
('brit-bennett', 'Brit Bennett', 'Bennett, Brit'),
('aimee-nezhukumatathil', 'Aimee Nezhukumatathil', 'Nezhukumatathil, Aimee'),
('markus-zusak', 'Markus Zusak', 'Zusak, Markus'),
('matt-haig', 'Matt Haig', 'Haig, Matt'),
('richard-powers', 'Richard Powers', 'Powers, Richard'),
('tara-westover', 'Tara Westover', 'Westover, Tara'),
('b-a-shapiro', 'B.A. Shapiro', 'Shapiro, B.A.'),
('min-jin-lee', 'Min Jin Lee', 'Lee, Min Jin'),
('erik-larson', 'Erik Larson', 'Larson, Erik'),
('madeline-miller', 'Madeline Miller', 'Miller, Madeline'),
('bessel-van-der-kolk', 'Bessel van der Kolk', 'van der Kolk, Bessel'),
('irma-s-rombauer', 'Irma S. Rombauer', 'Rombauer, Irma S.'),
('tj-klune', 'TJ Klune', 'Klune, TJ'),
('patrick-radden-keefe', 'Patrick Radden Keefe', 'Keefe, Patrick Radden'),
('v-e-schwab', 'V.E. Schwab', 'Schwab, V.E.'),
('james-clear', 'James Clear', 'Clear, James'),
('kirk-wallace-johnson', 'Kirk Wallace Johnson', 'Johnson, Kirk Wallace');

INSERT OR IGNORE INTO genres (slug, name) VALUES
('fiction', 'Fiction'),
('memoir', 'Memoir'),
('science-fiction', 'Science Fiction'),
('mystery', 'Mystery'),
('nature', 'Nature'),
('historical-fiction', 'Historical Fiction'),
('fantasy', 'Fantasy'),
('cookbooks', 'Cookbooks'),
('history', 'History'),
('psychology', 'Psychology'),
('business', 'Business');

INSERT OR IGNORE INTO books (id, slug, title, primary_author_id, primary_genre_id, year, isbn, publication_date, cover_color, aspect_ratio, tags, search_text, is_new_arrival) VALUES
('b001', 'the-night-circus', 'The Night Circus', (SELECT id FROM authors WHERE slug='erin-morgenstern'), (SELECT id FROM genres WHERE slug='fiction'), 2012, '9780307744432', '2012-01-01', '#1a2e26', 0.648, 'magical,atmospheric,romance', 'The Night Circus Erin Morgenstern Fiction magical atmospheric romance', 0),
('b002', 'kitchen-confidential', 'Kitchen Confidential', (SELECT id FROM authors WHERE slug='anthony-bourdain'), (SELECT id FROM genres WHERE slug='memoir'), 2007, '9780060899226', '2007-01-01', '#6b1827', 0.666, 'food,memoir,sharp', 'Kitchen Confidential Anthony Bourdain Memoir food memoir sharp', 1),
('b003', 'dune', 'Dune', (SELECT id FROM authors WHERE slug='frank-herbert'), (SELECT id FROM genres WHERE slug='science-fiction'), 2019, '9780441172719', '2019-01-01', '#a7482f', 1.0, 'classic,desert,politics', 'Dune Frank Herbert Science Fiction scifi sci-fi classic desert politics', 1),
('b004', 'the-secret-history', 'The Secret History', (SELECT id FROM authors WHERE slug='donna-tartt'), (SELECT id FROM genres WHERE slug='mystery'), 2004, '9781400031702', '2004-01-01', '#233e54', 0.648, 'campus,literary,dark', 'The Secret History Donna Tartt Mystery campus literary dark', 0),
('b005', 'braiding-sweetgrass', 'Braiding Sweetgrass', (SELECT id FROM authors WHERE slug='robin-wall-kimmerer'), (SELECT id FROM genres WHERE slug='nature'), 2020, '9781571313560', '2020-01-01', '#284234', 0.646, 'ecology,essays,indigenous', 'Braiding Sweetgrass Robin Wall Kimmerer Nature ecology essays indigenous', 0),
('b006', 'the-thursday-murder-club', 'The Thursday Murder Club', (SELECT id FROM authors WHERE slug='richard-osman'), (SELECT id FROM genres WHERE slug='mystery'), 2021, '9781984880987', '2021-01-01', '#b48c34', 0.666, 'cozy,witty,series', 'The Thursday Murder Club Richard Osman Mystery cozy witty series', 1),
('b007', 'a-gentleman-in-moscow', 'A Gentleman in Moscow', (SELECT id FROM authors WHERE slug='amor-towles'), (SELECT id FROM genres WHERE slug='historical-fiction'), 2019, '9780143110439', '2019-01-01', '#263e52', 0.638, 'elegant,hotel,historical', 'A Gentleman in Moscow Amor Towles Historical Fiction elegant hotel historical', 0),
('b008', 'the-hobbit', 'The Hobbit', (SELECT id FROM authors WHERE slug='j-r-r-tolkien'), (SELECT id FROM genres WHERE slug='fantasy'), 2001, '9780618260300', '2001-01-01', '#4e5933', 0.626, 'classic,adventure,quest', 'The Hobbit J.R.R. Tolkien Fantasy classic adventure quest', 0),
('b009', 'salt-fat-acid-heat', 'Salt Fat Acid Heat', (SELECT id FROM authors WHERE slug='samin-nosrat'), (SELECT id FROM genres WHERE slug='cookbooks'), 2017, '9781476753836', '2017-01-01', '#ab5433', 0.796, 'cooking,illustrated,technique', 'Salt Fat Acid Heat Samin Nosrat Cookbooks cooking illustrated technique', 1),
('b010', 'the-warmth-of-other-suns', 'The Warmth of Other Suns', (SELECT id FROM authors WHERE slug='isabel-wilkerson'), (SELECT id FROM genres WHERE slug='history'), 2011, '9780679763888', '2011-01-01', '#543928', 0.662, 'history,migration,america', 'The Warmth of Other Suns Isabel Wilkerson History migration america', 0),
('b011', 'project-hail-mary', 'Project Hail Mary', (SELECT id FROM authors WHERE slug='andy-weir'), (SELECT id FROM genres WHERE slug='science-fiction'), 2022, '9780593135228', '2022-01-01', '#214e63', 0.662, 'space,clever,survival', 'Project Hail Mary Andy Weir Science Fiction scifi sci-fi space clever survival', 1),
('b012', 'the-vanishing-half', 'The Vanishing Half', (SELECT id FROM authors WHERE slug='brit-bennett'), (SELECT id FROM genres WHERE slug='fiction'), 2021, '9780525536963', '2021-01-01', '#73334c', 0.640, 'family,identity,literary', 'The Vanishing Half Brit Bennett Fiction family identity literary', 0),
('b013', 'world-of-wonders', 'World of Wonders', (SELECT id FROM authors WHERE slug='aimee-nezhukumatathil'), (SELECT id FROM genres WHERE slug='nature'), 2020, '9781571313652', '2020-01-01', '#2b5c5a', 0.671, 'essays,nature,lyrical', 'World of Wonders Aimee Nezhukumatathil Nature essays lyrical', 0),
('b014', 'the-book-thief', 'The Book Thief', (SELECT id FROM authors WHERE slug='markus-zusak'), (SELECT id FROM genres WHERE slug='historical-fiction'), 2007, '9780375842207', '2007-01-01', '#3f4b54', 0.648, 'historical,young adult,moving', 'The Book Thief Markus Zusak Historical Fiction young adult moving', 0),
('b015', 'the-midnight-library', 'The Midnight Library', (SELECT id FROM authors WHERE slug='matt-haig'), (SELECT id FROM genres WHERE slug='fiction'), 2020, '9780525559474', '2020-01-01', '#204663', 0.662, 'reflective,second chances,bookish', 'The Midnight Library Matt Haig Fiction reflective second chances bookish', 1),
('b016', 'the-overstory', 'The Overstory', (SELECT id FROM authors WHERE slug='richard-powers'), (SELECT id FROM genres WHERE slug='fiction'), 2019, '9780393356687', '2019-01-01', '#2b522b', 0.666, 'trees,pulitzer,ambitious', 'The Overstory Richard Powers Fiction trees pulitzer ambitious', 0),
('b017', 'educated', 'Educated', (SELECT id FROM authors WHERE slug='tara-westover'), (SELECT id FROM genres WHERE slug='memoir'), 2020, '9780399590528', '2020-01-01', '#5c3d2e', 0.671, 'memoir,resilience,family', 'Educated Tara Westover Memoir resilience family', 0),
('b018', 'the-art-forger', 'The Art Forger', (SELECT id FROM authors WHERE slug='b-a-shapiro'), (SELECT id FROM genres WHERE slug='mystery'), 2013, '9781616203160', '2013-01-01', '#523b6b', 0.666, 'art,heist,boston', 'The Art Forger B.A. Shapiro Mystery art heist boston', 0),
('b019', 'pachinko', 'Pachinko', (SELECT id FROM authors WHERE slug='min-jin-lee'), (SELECT id FROM genres WHERE slug='historical-fiction'), 2017, '9781455563920', '2017-01-01', '#7d2b24', 0.658, 'family saga,korea,japan', 'Pachinko Min Jin Lee Historical Fiction family saga korea japan', 1),
('b020', 'the-splendid-and-the-vile', 'The Splendid and the Vile', (SELECT id FROM authors WHERE slug='erik-larson'), (SELECT id FROM genres WHERE slug='history'), 2020, '9780385348713', '2020-01-01', '#182d42', 0.656, 'churchill,wwii,narrative', 'The Splendid and the Vile Erik Larson History churchill wwii narrative', 0),
('b021', 'circe', 'Circe', (SELECT id FROM authors WHERE slug='madeline-miller'), (SELECT id FROM genres WHERE slug='fantasy'), 2019, '9780316556323', '2019-01-01', '#8c4f1c', 0.666, 'mythology,greek,literary', 'Circe Madeline Miller Fantasy mythology greek literary', 0),
('b022', 'the-body-keeps-the-score', 'The Body Keeps the Score', (SELECT id FROM authors WHERE slug='bessel-van-der-kolk'), (SELECT id FROM genres WHERE slug='psychology'), 2015, '9780143127741', '2015-01-01', '#2d444f', 0.650, 'psychology,trauma,health', 'The Body Keeps the Score Bessel van der Kolk Psychology trauma health', 0),
('b023', 'the-joy-of-cooking', 'The Joy of Cooking', (SELECT id FROM authors WHERE slug='irma-s-rombauer'), (SELECT id FROM genres WHERE slug='cookbooks'), 2006, '9780743246262', '2006-01-01', '#6b401d', 0.718, 'classic,reference,cooking', 'The Joy of Cooking Irma S. Rombauer Cookbooks classic reference cooking', 0),
('b024', 'the-house-in-the-cerulean-sea', 'The House in the Cerulean Sea', (SELECT id FROM authors WHERE slug='tj-klune'), (SELECT id FROM genres WHERE slug='fantasy'), 2021, '9781250217318', '2021-01-01', '#235c70', 0.642, 'warm,found family,hopeful', 'The House in the Cerulean Sea TJ Klune Fantasy warm found family hopeful', 1),
('b025', 'say-nothing', 'Say Nothing', (SELECT id FROM authors WHERE slug='patrick-radden-keefe'), (SELECT id FROM genres WHERE slug='history'), 2020, '9780307279286', '2020-01-01', '#323528', 0.648, 'ireland,true crime,politics', 'Say Nothing Patrick Radden Keefe History ireland true crime politics', 0),
('b026', 'the-invisible-life-of-addie-larue', 'The Invisible Life of Addie LaRue', (SELECT id FROM authors WHERE slug='v-e-schwab'), (SELECT id FROM genres WHERE slug='fantasy'), 2020, '9780765387561', '2020-01-01', '#13273b', 0.658, 'immortality,romance,paris', 'The Invisible Life of Addie LaRue V.E. Schwab Fantasy immortality romance paris', 1),
('b027', 'atomic-habits', 'Atomic Habits', (SELECT id FROM authors WHERE slug='james-clear'), (SELECT id FROM genres WHERE slug='business'), 2018, '9780735211292', '2018-01-01', '#162329', 0.662, 'habits,productivity,practical', 'Atomic Habits James Clear Business habits productivity practical', 0),
('b028', 'the-feather-thief', 'The Feather Thief', (SELECT id FROM authors WHERE slug='kirk-wallace-johnson'), (SELECT id FROM genres WHERE slug='mystery'), 2019, '9781101981634', '2019-01-01', '#2e4a46', 0.652, 'true crime,natural history,odd', 'The Feather Thief Kirk Wallace Johnson Mystery true crime natural history odd', 0);

INSERT OR IGNORE INTO book_authors (book_id, author_id, role, position)
SELECT id, primary_author_id, 'Author', 1 FROM books;

INSERT OR IGNORE INTO book_genres (book_id, genre_id, is_primary)
SELECT id, primary_genre_id, 1 FROM books;

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

INSERT OR IGNORE INTO book_collections (slug, name, description, cache_key, cache_ttl_seconds) VALUES
('best-sellers', 'Best Sellers', 'Popular copies readers keep grabbing from the used shelf.', 'collection:best-sellers', 300),
('staff-picks', 'Staff Picks', 'Books highlighted by Davis''s Books staff.', 'collection:staff-picks', 300),
('new-arrivals', 'New Arrivals', 'Recently inspected books added to the catalog.', 'collection:new-arrivals', 180),
('used-deals', 'Used Deals', 'Budget-friendly used books under $8.', 'collection:used-deals', 180);

INSERT OR IGNORE INTO book_collection_items (collection_slug, book_id, position) VALUES
('best-sellers', 'b005', 1),
('best-sellers', 'b003', 2),
('best-sellers', 'b024', 3),
('best-sellers', 'b004', 4),
('best-sellers', 'b009', 5),
('best-sellers', 'b019', 6),
('staff-picks', 'b001', 1),
('staff-picks', 'b003', 2),
('staff-picks', 'b004', 3),
('staff-picks', 'b005', 4),
('staff-picks', 'b007', 5),
('staff-picks', 'b009', 6),
('staff-picks', 'b013', 7),
('staff-picks', 'b016', 8),
('staff-picks', 'b019', 9),
('staff-picks', 'b021', 10),
('staff-picks', 'b024', 11),
('staff-picks', 'b028', 12),
('new-arrivals', 'b002', 1),
('new-arrivals', 'b003', 2),
('new-arrivals', 'b006', 3),
('new-arrivals', 'b009', 4),
('new-arrivals', 'b011', 5),
('new-arrivals', 'b015', 6),
('new-arrivals', 'b019', 7),
('new-arrivals', 'b024', 8),
('new-arrivals', 'b026', 9),
('used-deals', 'b018', 1),
('used-deals', 'b014', 2),
('used-deals', 'b008', 3),
('used-deals', 'b028', 4),
('used-deals', 'b013', 5),
('used-deals', 'b017', 6),
('used-deals', 'b006', 7);

INSERT OR IGNORE INTO cache_tags (tag, description)
SELECT 'book:' || b.id, 'Invalidate one public book detail page.' FROM books b;

INSERT OR IGNORE INTO cache_tags (tag, description)
SELECT 'author:' || a.slug, 'Invalidate author shelf and author-filtered catalog results.' FROM authors a;

INSERT OR IGNORE INTO cache_tags (tag, description)
SELECT 'genre:' || g.slug, 'Invalidate genre shelf and genre-filtered catalog results.' FROM genres g;

INSERT OR IGNORE INTO cache_tags (tag, description)
SELECT 'collection:' || c.slug, 'Invalidate merchandising shelf results.' FROM book_collections c;

INSERT OR IGNORE INTO book_cache_tags (book_id, tag)
SELECT b.id, 'book:' || b.id FROM books b;

INSERT OR IGNORE INTO book_cache_tags (book_id, tag)
SELECT b.id, 'author:' || a.slug
FROM books b
JOIN authors a ON a.id = b.primary_author_id;

INSERT OR IGNORE INTO book_cache_tags (book_id, tag)
SELECT b.id, 'genre:' || g.slug
FROM books b
JOIN genres g ON g.id = b.primary_genre_id;

INSERT OR IGNORE INTO book_cache_tags (book_id, tag)
SELECT i.book_id, 'collection:' || i.collection_slug
FROM book_collection_items i;
