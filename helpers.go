package main

import (
	"fmt"
	"net/http"
	"os"
	"strings"
)

func filtersFromRequest(r *http.Request) catalogFilters {
	q := r.URL.Query()
	return catalogFilters{
		Query:     strings.TrimSpace(q.Get("q")),
		Author:    strings.TrimSpace(q.Get("author")),
		Genre:     firstNonEmpty(q.Get("genre"), "All"),
		Condition: firstNonEmpty(q.Get("condition"), "All"),
		MaxPrice:  q.Get("max_price"),
		Format:    firstNonEmpty(q.Get("format"), "All"),
		Sort:      firstNonEmpty(q.Get("sort"), "popular"),
	}
}

func resultFilters(filters catalogFilters, shown, total int) catalogFilters {
	filters.ResultText = fmt.Sprintf("%d of %d used books shown", shown, total)
	return filters
}

func firstNonEmpty(value, fallback string) string {
	if strings.TrimSpace(value) == "" {
		return fallback
	}
	return value
}

func serveFile(path string) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		http.ServeFile(w, r, path)
	}
}

func env(key, fallback string) string {
	if value := strings.TrimSpace(os.Getenv(key)); value != "" {
		return value
	}
	return fallback
}

func uniqueGenres(books []bookCard) []string {
	seen := map[string]bool{}
	var genres []string
	for _, book := range books {
		if !seen[book.Genre] {
			seen[book.Genre] = true
			genres = append(genres, book.Genre)
		}
	}
	return genres
}

func uniqueConditions(books []bookCard) []string {
	seen := map[string]bool{}
	var conditions []string
	for _, book := range books {
		if !seen[book.Condition] {
			seen[book.Condition] = true
			conditions = append(conditions, book.Condition)
		}
	}
	return conditions
}

func uniqueFormats(books []bookCard) []string {
	seen := map[string]bool{}
	var formats []string
	for _, book := range books {
		if !seen[book.Format] {
			seen[book.Format] = true
			formats = append(formats, book.Format)
		}
	}
	return formats
}

func byIDs(books []bookCard, ids []string) []bookCard {
	lookup := map[string]bookCard{}
	for _, book := range books {
		lookup[book.ID] = book
	}
	var selected []bookCard
	for _, id := range ids {
		if book, ok := lookup[id]; ok {
			selected = append(selected, book)
		}
	}
	return selected
}

func mustBookByID(books []bookCard, id string) bookCard {
	for _, book := range books {
		if book.ID == id {
			return book
		}
	}
	if len(books) == 0 {
		return bookCard{}
	}
	return books[0]
}

func firstWhere(books []bookCard, limit int, keep func(bookCard) bool) []bookCard {
	var out []bookCard
	for _, book := range books {
		if keep(book) {
			out = append(out, book)
			if len(out) == limit {
				break
			}
		}
	}
	return out
}

func relatedBooks(books []bookCard, current bookCard, limit int) []bookCard {
	var related []bookCard
	seen := map[string]bool{current.ID: true}
	for _, book := range books {
		if book.Genre != current.Genre || seen[book.ID] {
			continue
		}
		related = append(related, book)
		seen[book.ID] = true
		if len(related) == limit {
			return related
		}
	}
	for _, book := range books {
		if seen[book.ID] {
			continue
		}
		related = append(related, book)
		seen[book.ID] = true
		if len(related) == limit {
			return related
		}
	}
	return related
}
