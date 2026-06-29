package main

import (
	"database/sql"
	"embed"
	"os"

	_ "modernc.org/sqlite"
)

//go:embed db/schema.sql
var schemaFS embed.FS

func openDB() (*sql.DB, error) {
	if err := os.MkdirAll("data", 0755); err != nil {
		return nil, err
	}
	dsn := env("DATABASE_URL", "file:data/bookstore.db?cache=shared&mode=rwc&_pragma=foreign_keys(1)")
	db, err := sql.Open("sqlite", dsn)
	if err != nil {
		return nil, err
	}
	db.SetMaxOpenConns(1)
	if err := db.Ping(); err != nil {
		return nil, err
	}
	return db, nil
}

func migrate(db *sql.DB) error {
	if err := resetLegacySchema(db); err != nil {
		return err
	}
	schema, err := schemaFS.ReadFile("db/schema.sql")
	if err != nil {
		return err
	}
	_, err = db.Exec(string(schema))
	return err
}

func resetLegacySchema(db *sql.DB) error {
	var count int
	if err := db.QueryRow(`SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'books'`).Scan(&count); err != nil {
		return err
	}
	if count == 0 {
		return nil
	}

	// Drop schema and re-migrate if variant_attributes is missing (merchandise release)
	var hasAttribs int
	if err := db.QueryRow(`SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'variant_attributes'`).Scan(&hasAttribs); err != nil {
		return err
	}
	if hasAttribs == 0 {
		_, err := db.Exec(`
			DROP TABLE IF EXISTS book_cache_tags;
			DROP TABLE IF EXISTS cache_tags;
			DROP TABLE IF EXISTS book_collection_items;
			DROP TABLE IF EXISTS book_collections;
			DROP TABLE IF EXISTS book_copies;
			DROP TABLE IF EXISTS book_genres;
			DROP TABLE IF EXISTS book_authors;
			DROP TABLE IF EXISTS books;
			DROP TABLE IF EXISTS genres;
			DROP TABLE IF EXISTS authors;
		`)
		return err
	}
	rows, err := db.Query(`PRAGMA table_info(books)`)
	if err != nil {
		return err
	}
	defer rows.Close()

	hasPrimaryAuthorID := false
	for rows.Next() {
		var cid int
		var name, columnType string
		var notNull int
		var defaultValue any
		var pk int
		if err := rows.Scan(&cid, &name, &columnType, &notNull, &defaultValue, &pk); err != nil {
			return err
		}
		if name == "primary_author_id" {
			hasPrimaryAuthorID = true
			break
		}
	}
	if err := rows.Err(); err != nil {
		return err
	}
	if hasPrimaryAuthorID {
		return nil
	}
	_, err = db.Exec(`
		DROP TABLE IF EXISTS book_cache_tags;
		DROP TABLE IF EXISTS cache_tags;
		DROP TABLE IF EXISTS book_collection_items;
		DROP TABLE IF EXISTS book_collections;
		DROP TABLE IF EXISTS book_copies;
		DROP TABLE IF EXISTS book_genres;
		DROP TABLE IF EXISTS book_authors;
		DROP TABLE IF EXISTS books;
		DROP TABLE IF EXISTS genres;
		DROP TABLE IF EXISTS authors;`)
	return err
}
