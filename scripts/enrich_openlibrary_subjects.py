#!/usr/bin/env python3
from __future__ import annotations

"""Enrich imported books with Open Library subjects and controlled genres.

The database stores the authoritative Open Library edition key in
``books.metadata_source_id``. This tool rehydrates that metadata, writes a
checkpoint JSONL report, and only changes the database when ``--apply`` is
provided. It never classifies a book from its title alone: unmatched records
remain in the generic ``Books`` genre.
"""

import argparse
import gzip
import json
import os
import subprocess
import time
from pathlib import Path
from urllib.request import Request, urlopen


SOURCE = "https://archive.org/download/ol_dump_2026-08-31/ol_dump_editions_2026-08-31.txt.gz"
SOURCE_NAME = "openlibrary-edition-bulk"
USER_AGENT = "ChantelsCornerCatalogEnricher/1.0 (bulk snapshot)"

# Specific categories must come before broad categories. These are deliberately
# small, transparent rules for the existing storefront taxonomy; raw subjects
# remain available for future mapping improvements.
GENRE_RULES = (
    (
        "science-fiction",
        ("science fiction", "sci-fi", "space fiction", "space opera"),
    ),
    (
        "historical-fiction",
        ("historical fiction", "historical novel", "historical romance"),
    ),
    ("fantasy", ("fantasy", "magic", "dragons", "fairy tales")),
    (
        "mystery",
        ("mystery", "detective", "crime", "thriller", "suspense"),
    ),
    (
        "religion",
        (
            "religion",
            "religious",
            "christianity",
            "christian",
            "islam",
            "judaism",
            "buddhism",
            "theology",
            "spirituality",
        ),
    ),
    ("cookbooks", ("cookbook", "cookery", "cooking", "recipes", "food")),
    (
        "psychology",
        ("psychology", "mental health", "psychotherapy", "trauma", "behavior"),
    ),
    (
        "business",
        (
            "business",
            "economics",
            "finance",
            "management",
            "marketing",
            "entrepreneur",
            "accounting",
        ),
    ),
    (
        "computers",
        (
            "computers",
            "computer science",
            "programming",
            "software",
            "internet",
            "information technology",
        ),
    ),
    (
        "technology",
        ("technology", "engineering", "industrial arts"),
    ),
    (
        "nature",
        (
            "nature",
            "ecology",
            "environment",
            "animals",
            "plants",
            "birds",
            "natural history",
        ),
    ),
    (
        "medicine",
        ("medical", "medicine", "nursing", "disease", "anatomy", "health"),
    ),
    (
        "science",
        ("science", "mathematics", "physics", "chemistry", "biology", "astronomy"),
    ),
    ("biography", ("biography", "biographical")),
    ("education", ("education", "teaching", "schools", "pedagogy")),
    ("reference", ("reference", "encyclopedias", "dictionaries")),
    ("travel", ("travel", "tourism", "guidebooks")),
    (
        "social-science",
        ("sociology", "social science", "anthropology", "social conditions"),
    ),
    ("music", ("music", "songbooks", "musical")),
    ("philosophy", ("philosophy", "ethics", "logic")),
    (
        "politics",
        ("politics", "political science", "government", "public policy"),
    ),
    ("memoir", ("memoir", "autobiography")),
    ("history", ("history", "historical")),
    (
        "fiction",
        (
            "fiction",
            "novels",
            "literature",
            "short stories",
            "stories",
            "drama",
            "poetry",
        ),
    ),
)


def database_url() -> str:
    value = os.environ.get("DATABASE_URL", "").strip()
    if not value:
        raise SystemExit("DATABASE_URL is required")
    return value


def psql(url: str, sql: str, input_text: str | None = None) -> str:
    command = ["psql", "-X", "-v", "ON_ERROR_STOP=1", url, "-At", "-F", "\t"]
    if input_text is None:
        command.extend(["-c", sql])
    else:
        command.extend(["-f", "-"])
    result = subprocess.run(
        command,
        input=input_text,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode:
        raise SystemExit(result.stderr.replace(url, "[database URL redacted]"))
    return result.stdout


def imported_books(url: str) -> list[dict[str, str]]:
    rows = psql(
        url,
        """
        SELECT id, metadata_source_id
        FROM books
        WHERE metadata_source = 'Open Library edition bulk dump'
          AND metadata_source_id LIKE '/books/%'
        ORDER BY id
        """,
    )
    return [
        {"book_id": book_id, "source_id": source_id}
        for book_id, source_id in (line.split("\t", 1) for line in rows.splitlines())
    ]


def clean_subject(value: object) -> str | None:
    if not isinstance(value, str):
        return None
    subject = " ".join(value.split()).strip(" .;:")
    return subject[:500] if subject else None


def mapped_genre(subjects: list[str]) -> str:
    lowered = [subject.casefold() for subject in subjects]
    for slug, needles in GENRE_RULES:
        if any(needle in subject for subject in lowered for needle in needles):
            return slug
    return "imported-books"


def bulk_subjects(items: list[dict[str, str]]) -> list[dict[str, object]]:
    """Match stored edition IDs against the pinned dump in one forward-only pass."""
    by_source_id = {item["source_id"]: item for item in items}
    rows: dict[str, dict[str, object]] = {}
    started = time.time()
    examined = 0
    request = Request(SOURCE, headers={"User-Agent": USER_AGENT})
    with urlopen(request, timeout=120) as response:
        with gzip.GzipFile(fileobj=response) as compressed:
            with compressed as raw_rows:
                for raw_line in raw_rows:
                    examined += 1
                    try:
                        payload = json.loads(raw_line.split(b"\t", 4)[-1])
                    except (UnicodeDecodeError, ValueError, json.JSONDecodeError):
                        continue
                    source_id = payload.get("key")
                    if source_id not in by_source_id:
                        if examined % 100_000 == 0:
                            print(
                                json.dumps(
                                    {
                                        "examined": examined,
                                        "matched": len(rows),
                                        "target": len(items),
                                        "seconds": int(time.time() - started),
                                    }
                                ),
                                flush=True,
                            )
                        continue
                    item = by_source_id[source_id]
                    subjects = sorted(
                        {
                            subject
                            for raw_subject in payload.get("subjects", [])
                            if (subject := clean_subject(raw_subject))
                        },
                        key=str.casefold,
                    )
                    rows[item["book_id"]] = {
                        **item,
                        "subjects": subjects,
                        "genre_slug": mapped_genre(subjects),
                        "status": "ok",
                    }
                    if len(rows) == len(items):
                        break
                    if examined % 100_000 == 0:
                        print(
                            json.dumps(
                                {
                                    "examined": examined,
                                    "matched": len(rows),
                                    "target": len(items),
                                    "seconds": int(time.time() - started),
                                }
                            ),
                            flush=True,
                        )

    for item in items:
        if item["book_id"] not in rows:
            rows[item["book_id"]] = {
                **item,
                "subjects": [],
                "genre_slug": "imported-books",
                "status": "missing_from_snapshot",
            }
    return sorted(rows.values(), key=lambda row: str(row["book_id"]))


def write_report(path: Path, rows: list[dict[str, object]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as output:
        for row in rows:
            output.write(json.dumps(row, ensure_ascii=False) + "\n")


def apply_rows(url: str, rows: list[dict[str, object]]) -> None:
    payload = json.dumps(rows, ensure_ascii=False, separators=(",", ":"))
    marker = "$enrichment$"
    sql = f"""
    BEGIN;
    CREATE TEMP TABLE incoming_subjects (
        book_id TEXT PRIMARY KEY,
        source_id TEXT NOT NULL,
        subjects JSONB NOT NULL,
        genre_slug TEXT NOT NULL
    ) ON COMMIT DROP;
    INSERT INTO incoming_subjects (book_id, source_id, subjects, genre_slug)
    SELECT book_id, source_id, subjects, genre_slug
    FROM jsonb_to_recordset({marker}{payload}{marker}::jsonb)
        AS rows(book_id TEXT, source_id TEXT, subjects JSONB, genre_slug TEXT);

    DELETE FROM book_subjects subjects
    USING incoming_subjects incoming
    WHERE subjects.book_id = incoming.book_id;

    INSERT INTO book_subjects (book_id, subject, source)
    SELECT incoming.book_id, subject.value, '{SOURCE_NAME}'
    FROM incoming_subjects incoming
    CROSS JOIN LATERAL jsonb_array_elements_text(incoming.subjects) AS subject(value)
    ON CONFLICT (book_id, subject) DO UPDATE SET source = EXCLUDED.source;

    DELETE FROM book_genres genres
    USING incoming_subjects incoming
    WHERE genres.book_id = incoming.book_id;

    INSERT INTO book_genres (book_id, genre_id, is_primary)
    SELECT incoming.book_id, genres.id, true
    FROM incoming_subjects incoming
    JOIN genres ON genres.slug = incoming.genre_slug;

    UPDATE books
    SET primary_genre_id = genres.id,
        tags = COALESCE(subjects.tags, ''),
        search_text = books.search_text
    FROM incoming_subjects incoming
    JOIN genres ON genres.slug = incoming.genre_slug
    LEFT JOIN LATERAL (
        SELECT string_agg(subject.value, ', ' ORDER BY subject.value) AS tags
        FROM jsonb_array_elements_text(incoming.subjects) AS subject(value)
    ) subjects ON true
    WHERE books.id = incoming.book_id;
    COMMIT;
    """
    psql(url, "", input_text=sql)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--apply", action="store_true", help="write the fetched enrichment to Postgres")
    parser.add_argument("--limit", type=int, help="only process the first N imported books")
    parser.add_argument(
        "--workers",
        type=int,
        default=8,
        help="accepted for compatibility; the pinned gzip snapshot is streamed serially",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("setup/artifacts/book-import/openlibrary-enrichment.jsonl"),
    )
    args = parser.parse_args()
    if args.workers < 1 or args.workers > 16:
        raise SystemExit("--workers must be between 1 and 16")

    url = database_url()
    items = imported_books(url)
    if args.limit:
        items = items[: args.limit]
    if not items:
        raise SystemExit("No imported Open Library books found")

    if args.workers != 8:
        print(json.dumps({"notice": "--workers is ignored for the single streaming bulk snapshot"}), flush=True)
    rows = bulk_subjects(items)
    write_report(args.output, rows)

    statuses: dict[str, int] = {}
    genres: dict[str, int] = {}
    for row in rows:
        statuses[str(row["status"])] = statuses.get(str(row["status"]), 0) + 1
        genres[str(row["genre_slug"])] = genres.get(str(row["genre_slug"]), 0) + 1
    print(json.dumps({"statuses": statuses, "genres": genres, "output": str(args.output)}))

    if args.apply:
        failed = [row for row in rows if row["status"] != "ok"]
        if failed:
            raise SystemExit(f"Refusing partial apply; {len(failed)} records were missing from the pinned snapshot")
        apply_rows(url, rows)
        print(json.dumps({"applied": len(rows)}))


if __name__ == "__main__":
    main()
