package main

import (
	"database/sql"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/alexedwards/scs/v2"
	_ "modernc.org/sqlite"
)

func newTestApp(t *testing.T) *application {
	t.Helper()

	db, err := sql.Open("sqlite", "file:testdb?mode=memory&cache=shared&_pragma=foreign_keys(1)")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = db.Close() })
	db.SetMaxOpenConns(1)
	if err := migrate(db); err != nil {
		t.Fatal(err)
	}

	tmpl, err := parseTemplates()
	if err != nil {
		t.Fatal(err)
	}

	sessionManager := scs.New()
	sessionManager.Lifetime = time.Hour
	sessionManager.Cookie.Name = "davis_books_session"
	sessionManager.Cookie.HttpOnly = true
	sessionManager.Cookie.SameSite = http.SameSiteLaxMode

	return &application{
		db:        db,
		sessions:  sessionManager,
		templates: tmpl,
	}
}

func TestHomeRendersServerCatalog(t *testing.T) {
	app := newTestApp(t)
	req := httptest.NewRequest(http.MethodGet, "/", nil)
	rr := httptest.NewRecorder()

	app.routes().ServeHTTP(rr, req)

	if rr.Code != http.StatusOK {
		t.Fatalf("status = %d, want %d", rr.Code, http.StatusOK)
	}
	body := rr.Body.String()
	assertContains(t, body, "Davis&#39;s Books | Used Books Online")
	assertContains(t, body, `id="catalogResults"`)
	assertContains(t, body, `class="hero-overlay-grid"`)
	assertContains(t, body, "Shop by Category")
	assertContains(t, body, "Stack Builder")
	assertContains(t, body, "The Night Circus")
	assertContains(t, body, `<script src="/assets/htmx.min.js" defer></script>`)
	assertContains(t, body, `<script src="/app.js"></script>`)
	if strings.Contains(body, `<script src="/script.js"></script>`) {
		t.Fatal("home should not load legacy mock-data script.js")
	}
	if strings.Contains(body, "unpkg.com/htmx") {
		t.Fatal("home should use the vendored HTMX asset")
	}
}

func TestBookDetailPageRenders(t *testing.T) {
	app := newTestApp(t)
	req := httptest.NewRequest(http.MethodGet, "/books/b003", nil)
	rr := httptest.NewRecorder()

	app.routes().ServeHTTP(rr, req)

	if rr.Code != http.StatusOK {
		t.Fatalf("status = %d, want %d", rr.Code, http.StatusOK)
	}
	body := rr.Body.String()
	assertContains(t, body, "Dune | Davis&#39;s Books")
	assertContains(t, body, "Tight binding, light page tanning, no markings.")
	assertContains(t, body, "Add to Stack")
	assertContains(t, body, "Keep browsing this shelf")
	assertContains(t, body, `href="/#best-sellers"`)
	assertContains(t, body, `href="/#new-arrivals"`)
	assertContains(t, body, `href="/#deals"`)
	assertContains(t, body, `href="/#staff-picks"`)
	assertContains(t, body, `href="/#sell"`)
	assertContains(t, body, `href="/#catalog"`)
	assertContains(t, body, `action="/#catalog" method="get"`)
	assertContains(t, body, `name="genre"`)
	if strings.Contains(body, `href="#best-sellers"`) || strings.Contains(body, `href="#sell"`) {
		t.Fatal("detail page should use store-root section links, not product-page-local hash links")
	}
	if strings.Contains(body, `hx-target="#catalogResults"`) {
		t.Fatal("detail page header search should not depend on homepage-only HTMX targets")
	}
}

func TestCatalogHTMXFiltersByQuery(t *testing.T) {
	app := newTestApp(t)
	req := httptest.NewRequest(http.MethodGet, "/catalog?q=dune", nil)
	req.Header.Set("HX-Request", "true")
	rr := httptest.NewRecorder()

	app.routes().ServeHTTP(rr, req)

	if rr.Code != http.StatusOK {
		t.Fatalf("status = %d, want %d", rr.Code, http.StatusOK)
	}
	body := rr.Body.String()
	assertContains(t, body, "1 of 31 used books shown")
	assertContains(t, body, "Dune")
	if strings.Contains(body, "The Night Circus") {
		t.Fatal("filtered catalog fragment included an unrelated title")
	}
}

func TestCatalogFiltersByAuthorSlug(t *testing.T) {
	app := newTestApp(t)
	req := httptest.NewRequest(http.MethodGet, "/catalog?author=frank-herbert", nil)
	req.Header.Set("HX-Request", "true")
	rr := httptest.NewRecorder()

	app.routes().ServeHTTP(rr, req)

	if rr.Code != http.StatusOK {
		t.Fatalf("status = %d, want %d", rr.Code, http.StatusOK)
	}
	body := rr.Body.String()
	assertContains(t, body, "1 of 31 used books shown")
	assertContains(t, body, "Dune")
	if strings.Contains(body, "Project Hail Mary") {
		t.Fatal("author-filtered catalog fragment included another science fiction author")
	}
}

func TestSchemaSupportsCacheTargets(t *testing.T) {
	app := newTestApp(t)

	var authorSlug, genreSlug, collectionCacheKey string
	err := app.db.QueryRow(`
		SELECT a.slug, g.slug, c.cache_key
		FROM books b
		JOIN authors a ON a.id = b.primary_author_id
		JOIN genres g ON g.id = b.primary_genre_id
		JOIN book_collection_items i ON i.book_id = b.id
		JOIN book_collections c ON c.slug = i.collection_slug
		WHERE b.id = 'b003' AND c.slug = 'best-sellers'`).Scan(&authorSlug, &genreSlug, &collectionCacheKey)
	if err != nil {
		t.Fatal(err)
	}
	if authorSlug != "frank-herbert" || genreSlug != "science-fiction" || collectionCacheKey != "collection:best-sellers" {
		t.Fatalf("cache target fields = %q, %q, %q", authorSlug, genreSlug, collectionCacheKey)
	}

	for _, tag := range []string{"book:b003", "author:frank-herbert", "genre:science-fiction", "collection:best-sellers"} {
		var count int
		if err := app.db.QueryRow(`SELECT COUNT(*) FROM book_cache_tags WHERE book_id = 'b003' AND tag = ?`, tag).Scan(&count); err != nil {
			t.Fatal(err)
		}
		if count != 1 {
			t.Fatalf("cache tag %q count = %d, want 1", tag, count)
		}
	}
}

func TestCartSessionPersistsAndCapsAtStock(t *testing.T) {
	app := newTestApp(t)
	handler := app.routes()

	var body string
	var cookies []*http.Cookie
	for range 5 {
		req := httptest.NewRequest(http.MethodPost, "/cart/items", strings.NewReader("copy_id=1"))
		req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
		for _, cookie := range cookies {
			req.AddCookie(cookie)
		}
		rr := httptest.NewRecorder()
		handler.ServeHTTP(rr, req)
		if rr.Code != http.StatusOK {
			t.Fatalf("status = %d, want %d; body=%s", rr.Code, http.StatusOK, rr.Body.String())
		}
		body = rr.Body.String()
		cookies = rr.Result().Cookies()
	}

	assertContains(t, body, `data-cart-count="3"`)
	assertContains(t, body, "The Night Circus")
	assertContains(t, body, "$28.50")

	req := httptest.NewRequest(http.MethodPost, "/checkout", nil)
	for _, cookie := range cookies {
		req.AddCookie(cookie)
	}
	rr := httptest.NewRecorder()
	handler.ServeHTTP(rr, req)
	checkoutBody := rr.Body.String()
	if rr.Code != http.StatusOK {
		t.Fatalf("checkout status = %d, want %d; body=%s", rr.Code, http.StatusOK, checkoutBody)
	}
	assertContains(t, checkoutBody, "Cart total: $33.45")
}

func assertContains(t *testing.T, body, needle string) {
	t.Helper()
	if !strings.Contains(body, needle) {
		t.Fatalf("expected body to contain %q", needle)
	}
}
