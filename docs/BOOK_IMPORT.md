# Bulk catalog import

Use Open Library's [monthly bulk dumps](https://openlibrary.org/developers/dumps)
and [bulk cover metadata](https://openlibrary.org/dev/docs/api/covers). The
reproducible source snapshot is August 31, 2026, hosted on Archive.org. Its catalog
reuse terms are described in the [Open Library FAQ](https://openlibrary.org/help/faq/using).
The UCSD Goodreads dataset was rejected because its publisher prohibits redistribution.

```sh
python3 scripts/collect_openlibrary.py --count 13000
curl -fL https://archive.org/download/ol_dump_2026-08-31/ol_dump_covers_metadata_2026-08-31.txt.gz -o setup/artifacts/book-import/covers-metadata.txt.gz
python3 scripts/import_books.py prepare --candidates setup/artifacts/book-import/candidates.jsonl --cover-metadata setup/artifacts/book-import/covers-metadata.txt.gz --count 10000
DATABASE_URL='postgresql://USER@127.0.0.1:PORT/DATABASE' python3 scripts/import_books.py apply --count 10000
```

Python Pillow and PostgreSQL `psql` are required. Apply migrations through the
application's SQLx migrator before import. Generated manifests and reports live
in ignored `setup/artifacts/book-import/`. Preparation performs no database
writes. A failed target count refuses import. Apply uses a transaction, checks
ISBN collisions against existing records, and is idempotent for the same
manifest. External databases require `--allow-database-host EXACT_HOST`; this
flag should only be used for an explicitly authorized destination.

Every accepted edition has a checksum-valid ISBN, title, author by-statement,
title and a positive assigned price. Sparse description, author, publisher,
year, and format fields are backfilled with honest `Unknown`/`unavailable`
values so templates and HTMX always receive strings. Its cover URL is fetched
and decoded as an actual image with minimum dimensions before acceptance.
Missing binding information
is labeled `Format unspecified`; no binding is invented. Source descriptions
are stripped of HTML and rendered as escaped text. Books without enough
metadata are rejected.

Prices are **assigned demo USD prices**: $14.99 hardcover or $9.99 for other
formats. They are not market offers. Stock is zero; no physical inventory is
fabricated. No compare-at list prices are invented. Metadata and price source
are stored separately from product content.

Cover selection starts with official bulk metadata, then fetches each cover URL
and decodes the returned image: each accepted image must be at least 70 pixels
wide and 100 pixels high. The importer does not crawl the Covers API. An
alternative `--editions PATH --covers DIRECTORY` prepare mode
validates decoded image bytes from an extracted bulk cover archive.

Covers are displayed through Open Library URLs, with attribution on book detail
pages. Application and database changes must ship together for descriptions and
remote cover URLs to render; existing books retain their local cover paths.
