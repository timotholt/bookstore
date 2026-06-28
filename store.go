package main

import (
	"strconv"
	"strings"
)

func (app *application) listBooks(filters catalogFilters) ([]bookCard, error) {
	const base = `
		SELECT
			b.id, b.title, a.name, a.slug, g.name, g.slug, b.year, b.isbn, b.cover_color,
			b.aspect_ratio, b.tags, b.is_new_arrival,
			c.id, c.condition, c.price, c.notes, c.format, c.stock,
			c.is_staff_pick, COALESCE(c.staff_quote, ''), c.seal_style, c.seal_text
		FROM books b
		JOIN authors a ON a.id = b.primary_author_id
		JOIN genres g ON g.id = b.primary_genre_id
		JOIN book_copies c ON c.book_id = b.id
		WHERE c.is_sold = 0`

	args := []any{}
	where := ""
	if strings.TrimSpace(filters.Query) != "" {
		where = ` AND (
			lower(b.search_text) LIKE lower(?)
			OR lower(a.name) LIKE lower(?)
			OR lower(g.name) LIKE lower(?)
			OR b.isbn LIKE ?
			OR lower(b.tags) LIKE lower(?)
		)`
		like := "%" + strings.TrimSpace(filters.Query) + "%"
		args = append(args, like, like, like, like, like)
	}
	if filters.Author != "" {
		where += " AND (a.slug = ? OR a.name = ?)"
		args = append(args, filters.Author, filters.Author)
	}
	if filters.Genre != "" && filters.Genre != "All" {
		where += " AND (g.slug = ? OR g.name = ?)"
		args = append(args, filters.Genre, filters.Genre)
	}
	if filters.Condition != "" && filters.Condition != "All" {
		where += " AND c.condition = ?"
		args = append(args, filters.Condition)
	}
	if filters.Format != "" && filters.Format != "All" {
		where += " AND c.format = ?"
		args = append(args, filters.Format)
	}
	if filters.MaxPrice != "" {
		maxPrice, err := strconv.ParseFloat(filters.MaxPrice, 64)
		if err == nil && maxPrice > 0 {
			where += " AND c.price <= ?"
			args = append(args, maxPrice)
		}
	}

	order := " ORDER BY c.is_staff_pick DESC, b.is_new_arrival DESC, b.title ASC"
	switch filters.Sort {
	case "price-asc":
		order = " ORDER BY c.price ASC, b.title ASC"
	case "price-desc":
		order = " ORDER BY c.price DESC, b.title ASC"
	case "year-desc":
		order = " ORDER BY b.year DESC, b.title ASC"
	case "title":
		order = " ORDER BY b.title ASC"
	}

	rows, err := app.db.Query(base+where+order, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var books []bookCard
	for rows.Next() {
		var b bookCard
		if err := rows.Scan(
			&b.ID, &b.Title, &b.Author, &b.AuthorSlug, &b.Genre, &b.GenreSlug, &b.Year, &b.ISBN, &b.CoverColor,
			&b.AspectRatio, &b.Tags, &b.IsNewArrival,
			&b.CopyID, &b.Condition, &b.Price, &b.Notes, &b.Format, &b.Stock,
			&b.IsStaffPick, &b.StaffQuote, &b.SealStyle, &b.SealText,
		); err != nil {
			return nil, err
		}
		books = append(books, b)
	}
	return books, rows.Err()
}

func (app *application) collectionBooks(slug string, limit int) ([]bookCard, error) {
	const query = `
		SELECT
			b.id, b.title, a.name, a.slug, g.name, g.slug, b.year, b.isbn, b.cover_color,
			b.aspect_ratio, b.tags, b.is_new_arrival,
			c.id, c.condition, c.price, c.notes, c.format, c.stock,
			c.is_staff_pick, COALESCE(c.staff_quote, ''), c.seal_style, c.seal_text
		FROM book_collection_items i
		JOIN books b ON b.id = i.book_id
		JOIN authors a ON a.id = b.primary_author_id
		JOIN genres g ON g.id = b.primary_genre_id
		JOIN book_copies c ON c.book_id = b.id
		WHERE i.collection_slug = ? AND i.is_active = 1 AND c.is_sold = 0
		ORDER BY i.position ASC, c.is_staff_pick DESC, c.price ASC
		LIMIT ?`
	rows, err := app.db.Query(query, slug, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var books []bookCard
	for rows.Next() {
		var b bookCard
		if err := rows.Scan(
			&b.ID, &b.Title, &b.Author, &b.AuthorSlug, &b.Genre, &b.GenreSlug, &b.Year, &b.ISBN, &b.CoverColor,
			&b.AspectRatio, &b.Tags, &b.IsNewArrival,
			&b.CopyID, &b.Condition, &b.Price, &b.Notes, &b.Format, &b.Stock,
			&b.IsStaffPick, &b.StaffQuote, &b.SealStyle, &b.SealText,
		); err != nil {
			return nil, err
		}
		books = append(books, b)
	}
	return books, rows.Err()
}

func (app *application) bookByCopyID(copyID int64) (bookCard, error) {
	row := app.db.QueryRow(`
		SELECT
			b.id, b.title, a.name, a.slug, g.name, g.slug, b.year, b.isbn, b.cover_color,
			b.aspect_ratio, b.tags, b.is_new_arrival,
			c.id, c.condition, c.price, c.notes, c.format, c.stock,
			c.is_staff_pick, COALESCE(c.staff_quote, ''), c.seal_style, c.seal_text
		FROM books b
		JOIN authors a ON a.id = b.primary_author_id
		JOIN genres g ON g.id = b.primary_genre_id
		JOIN book_copies c ON c.book_id = b.id
		WHERE c.id = ? AND c.is_sold = 0`, copyID)
	var b bookCard
	err := row.Scan(
		&b.ID, &b.Title, &b.Author, &b.AuthorSlug, &b.Genre, &b.GenreSlug, &b.Year, &b.ISBN, &b.CoverColor,
		&b.AspectRatio, &b.Tags, &b.IsNewArrival,
		&b.CopyID, &b.Condition, &b.Price, &b.Notes, &b.Format, &b.Stock,
		&b.IsStaffPick, &b.StaffQuote, &b.SealStyle, &b.SealText,
	)
	return b, err
}

func (app *application) bookByID(bookID string) (bookCard, error) {
	row := app.db.QueryRow(`
		SELECT
			b.id, b.title, a.name, a.slug, g.name, g.slug, b.year, b.isbn, b.cover_color,
			b.aspect_ratio, b.tags, b.is_new_arrival,
			c.id, c.condition, c.price, c.notes, c.format, c.stock,
			c.is_staff_pick, COALESCE(c.staff_quote, ''), c.seal_style, c.seal_text
		FROM books b
		JOIN authors a ON a.id = b.primary_author_id
		JOIN genres g ON g.id = b.primary_genre_id
		JOIN book_copies c ON c.book_id = b.id
		WHERE b.id = ? AND c.is_sold = 0
		ORDER BY c.is_staff_pick DESC, c.price ASC
		LIMIT 1`, bookID)
	var b bookCard
	err := row.Scan(
		&b.ID, &b.Title, &b.Author, &b.AuthorSlug, &b.Genre, &b.GenreSlug, &b.Year, &b.ISBN, &b.CoverColor,
		&b.AspectRatio, &b.Tags, &b.IsNewArrival,
		&b.CopyID, &b.Condition, &b.Price, &b.Notes, &b.Format, &b.Stock,
		&b.IsStaffPick, &b.StaffQuote, &b.SealStyle, &b.SealText,
	)
	return b, err
}

func (app *application) copyStock(copyID int64) (int, error) {
	var stock int
	err := app.db.QueryRow(`SELECT stock FROM book_copies WHERE id = ? AND is_sold = 0`, copyID).Scan(&stock)
	return stock, err
}
