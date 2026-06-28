package main

import (
	"database/sql"
	"embed"
	"encoding/gob"
	"fmt"
	"html/template"
	"log"
	"math"
	"net/http"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"time"

	"github.com/alexedwards/scs/v2"
	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
	_ "modernc.org/sqlite"
)

//go:embed db/schema.sql
var schemaFS embed.FS

type application struct {
	db        *sql.DB
	sessions  *scs.SessionManager
	templates *template.Template
}

type bookCard struct {
	ID           string
	Title        string
	Author       string
	Genre        string
	Year         int
	ISBN         string
	CoverColor   string
	AspectRatio  float64
	Tags         string
	IsNewArrival bool
	CopyID       int64
	Condition    string
	Price        float64
	Notes        string
	Format       string
	Stock        int
	IsStaffPick  bool
	StaffQuote   string
	SealStyle    string
	SealText     string
}

type homePageData struct {
	Title        string
	Genres       []string
	Conditions   []string
	Formats      []string
	Featured     bookCard
	QuickFillers []bookCard
	BestSellers  []bookCard
	NewArrivals  []bookCard
	Deals        []bookCard
	Catalog      []bookCard
	StaffPicks   []bookCard
	Cart         cartView
	Filters      catalogFilters
}

type bookDetailPageData struct {
	Title   string
	Genres  []string
	Book    bookCard
	Related []bookCard
	Cart    cartView
}

type catalogFilters struct {
	Query      string
	Genre      string
	Condition  string
	MaxPrice   string
	Format     string
	Sort       string
	ResultText string
}

type cartItem struct {
	CopyID   int64
	Quantity int
}

type cartLine struct {
	Book      bookCard
	Quantity  int
	LineTotal float64
}

type cartView struct {
	Lines         []cartLine
	ItemCount     int
	Subtotal      float64
	Shipping      float64
	Total         float64
	FreeShipping  bool
	ProgressText  string
	ProgressRatio float64
}

func init() {
	gob.Register([]cartItem{})
}

func main() {
	db, err := openDB()
	if err != nil {
		log.Fatal(err)
	}
	defer db.Close()

	if err := migrate(db); err != nil {
		log.Fatal(err)
	}

	tmpl, err := parseTemplates()
	if err != nil {
		log.Fatal(err)
	}

	sessionManager := scs.New()
	sessionManager.Lifetime = 24 * time.Hour
	sessionManager.Cookie.Name = "davis_books_session"
	sessionManager.Cookie.HttpOnly = true
	sessionManager.Cookie.SameSite = http.SameSiteLaxMode
	sessionManager.Cookie.Secure = os.Getenv("APP_ENV") == "production"

	app := &application{
		db:        db,
		sessions:  sessionManager,
		templates: tmpl,
	}

	srv := &http.Server{
		Addr:         env("ADDR", ":8080"),
		Handler:      app.routes(),
		ReadTimeout:  10 * time.Second,
		WriteTimeout: 10 * time.Second,
		IdleTimeout:  60 * time.Second,
	}

	log.Printf("Davis's Books listening on http://localhost%s", srv.Addr)
	log.Fatal(srv.ListenAndServe())
}

func (app *application) routes() http.Handler {
	r := chi.NewRouter()
	r.Use(middleware.RequestID)
	r.Use(middleware.RealIP)
	r.Use(middleware.Logger)
	r.Use(middleware.Recoverer)
	r.Use(app.sessions.LoadAndSave)

	r.Get("/", app.home)
	r.Get("/catalog", app.catalog)
	r.Get("/books/{bookID}", app.bookDetail)
	r.Post("/cart/items", app.addCartItem)
	r.Post("/cart/items/{copyID}/increase", app.increaseCartItem)
	r.Post("/cart/items/{copyID}/decrease", app.decreaseCartItem)
	r.Post("/cart/items/{copyID}/remove", app.removeCartItem)
	r.Post("/checkout", app.checkout)
	r.Get("/healthz", func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusNoContent)
	})

	r.Handle("/assets/*", http.StripPrefix("/assets/", http.FileServer(http.Dir("./assets"))))
	r.Get("/styles.css", serveFile("styles.css"))
	r.Get("/app.js", serveFile("app.js"))

	return r
}

func (app *application) home(w http.ResponseWriter, r *http.Request) {
	filters := filtersFromRequest(r)
	books, err := app.listBooks(filters)
	if err != nil {
		http.Error(w, "catalog unavailable", http.StatusInternalServerError)
		return
	}
	allBooks, err := app.listBooks(catalogFilters{})
	if err != nil {
		http.Error(w, "catalog unavailable", http.StatusInternalServerError)
		return
	}
	cart, err := app.cartView(r)
	if err != nil {
		http.Error(w, "cart unavailable", http.StatusInternalServerError)
		return
	}

	data := homePageData{
		Title:        "Davis's Books | Used Books Online",
		Genres:       uniqueGenres(allBooks),
		Conditions:   uniqueConditions(allBooks),
		Formats:      uniqueFormats(allBooks),
		Featured:     mustBookByID(allBooks, "b005"),
		QuickFillers: firstWhere(allBooks, 2, func(b bookCard) bool { return b.Price < 8 }),
		BestSellers:  byIDs(allBooks, []string{"b005", "b003", "b024", "b004", "b009", "b019"}),
		NewArrivals:  firstWhere(allBooks, 6, func(b bookCard) bool { return b.IsNewArrival }),
		Deals:        firstWhere(allBooks, 6, func(b bookCard) bool { return b.Price < 8 }),
		Catalog:      books,
		StaffPicks:   firstWhere(allBooks, 3, func(b bookCard) bool { return b.IsStaffPick }),
		Cart:         cart,
		Filters:      resultFilters(filters, len(books), len(allBooks)),
	}

	if err := app.templates.ExecuteTemplate(w, "base", data); err != nil {
		http.Error(w, "template error", http.StatusInternalServerError)
	}
}

func (app *application) catalog(w http.ResponseWriter, r *http.Request) {
	filters := filtersFromRequest(r)
	books, err := app.listBooks(filters)
	if err != nil {
		http.Error(w, "catalog unavailable", http.StatusInternalServerError)
		return
	}
	allBooks, err := app.listBooks(catalogFilters{})
	if err != nil {
		http.Error(w, "catalog unavailable", http.StatusInternalServerError)
		return
	}

	if r.Header.Get("HX-Request") == "true" {
		data := struct {
			Catalog []bookCard
			Filters catalogFilters
		}{
			Catalog: books,
			Filters: resultFilters(filters, len(books), len(allBooks)),
		}
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		if err := app.templates.ExecuteTemplate(w, "catalog-results", data); err != nil {
			http.Error(w, "template error", http.StatusInternalServerError)
		}
		return
	}

	http.Redirect(w, r, "/#catalog", http.StatusSeeOther)
}

func (app *application) bookDetail(w http.ResponseWriter, r *http.Request) {
	bookID := chi.URLParam(r, "bookID")
	book, err := app.bookByID(bookID)
	if err != nil {
		if err == sql.ErrNoRows {
			http.NotFound(w, r)
			return
		}
		http.Error(w, "book unavailable", http.StatusInternalServerError)
		return
	}
	allBooks, err := app.listBooks(catalogFilters{})
	if err != nil {
		http.Error(w, "catalog unavailable", http.StatusInternalServerError)
		return
	}
	cart, err := app.cartView(r)
	if err != nil {
		http.Error(w, "cart unavailable", http.StatusInternalServerError)
		return
	}
	data := bookDetailPageData{
		Title:   book.Title + " | Davis's Books",
		Genres:  uniqueGenres(allBooks),
		Book:    book,
		Related: relatedBooks(allBooks, book, 4),
		Cart:    cart,
	}
	if err := app.templates.ExecuteTemplate(w, "book-detail-base", data); err != nil {
		http.Error(w, "template error", http.StatusInternalServerError)
	}
}

func (app *application) addCartItem(w http.ResponseWriter, r *http.Request) {
	if err := r.ParseForm(); err != nil {
		http.Error(w, "invalid cart request", http.StatusBadRequest)
		return
	}
	copyID, err := strconv.ParseInt(r.FormValue("copy_id"), 10, 64)
	if err != nil || copyID < 1 {
		http.Error(w, "invalid copy", http.StatusBadRequest)
		return
	}
	items := app.cartItems(r)
	stock, err := app.copyStock(copyID)
	if err != nil {
		http.Error(w, "copy unavailable", http.StatusNotFound)
		return
	}
	items = setCartQuantity(items, copyID, min(quantityFor(items, copyID)+1, stock))
	app.sessions.Put(r.Context(), "cart", items)
	app.renderCart(w, r)
}

func (app *application) increaseCartItem(w http.ResponseWriter, r *http.Request) {
	app.changeCartQuantity(w, r, 1)
}

func (app *application) decreaseCartItem(w http.ResponseWriter, r *http.Request) {
	app.changeCartQuantity(w, r, -1)
}

func (app *application) removeCartItem(w http.ResponseWriter, r *http.Request) {
	copyID, err := strconv.ParseInt(chi.URLParam(r, "copyID"), 10, 64)
	if err != nil {
		http.Error(w, "invalid copy", http.StatusBadRequest)
		return
	}
	app.sessions.Put(r.Context(), "cart", setCartQuantity(app.cartItems(r), copyID, 0))
	app.renderCart(w, r)
}

func (app *application) changeCartQuantity(w http.ResponseWriter, r *http.Request, delta int) {
	copyID, err := strconv.ParseInt(chi.URLParam(r, "copyID"), 10, 64)
	if err != nil {
		http.Error(w, "invalid copy", http.StatusBadRequest)
		return
	}
	items := app.cartItems(r)
	nextQuantity := quantityFor(items, copyID) + delta
	if nextQuantity > 0 {
		stock, err := app.copyStock(copyID)
		if err != nil {
			http.Error(w, "copy unavailable", http.StatusNotFound)
			return
		}
		nextQuantity = min(nextQuantity, stock)
	}
	items = setCartQuantity(items, copyID, nextQuantity)
	app.sessions.Put(r.Context(), "cart", items)
	app.renderCart(w, r)
}

func (app *application) checkout(w http.ResponseWriter, r *http.Request) {
	cart, err := app.cartView(r)
	if err != nil {
		http.Error(w, "cart unavailable", http.StatusInternalServerError)
		return
	}
	if cart.ItemCount == 0 {
		http.Error(w, "cart is empty", http.StatusBadRequest)
		return
	}
	w.Header().Set("Content-Type", "text/plain; charset=utf-8")
	fmt.Fprintf(w, "Stripe Checkout will be connected in Phase 4. Cart total: %s", formatMoney(cart.Total))
}

func (app *application) renderCart(w http.ResponseWriter, r *http.Request) {
	cart, err := app.cartView(r)
	if err != nil {
		http.Error(w, "cart unavailable", http.StatusInternalServerError)
		return
	}
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	if err := app.templates.ExecuteTemplate(w, "cart-drawer", cart); err != nil {
		http.Error(w, "template error", http.StatusInternalServerError)
	}
}

func (app *application) listBooks(filters catalogFilters) ([]bookCard, error) {
	const base = `
		SELECT
			b.id, b.title, b.author, b.genre, b.year, b.isbn, b.cover_color,
			b.aspect_ratio, b.tags, b.is_new_arrival,
			c.id, c.condition, c.price, c.notes, c.format, c.stock,
			c.is_staff_pick, COALESCE(c.staff_quote, ''), c.seal_style, c.seal_text
		FROM books b
		JOIN book_copies c ON c.book_id = b.id
		WHERE c.is_sold = 0`

	args := []any{}
	where := ""
	if strings.TrimSpace(filters.Query) != "" {
		where = ` AND (
			lower(b.title) LIKE lower(?)
			OR lower(b.author) LIKE lower(?)
			OR lower(b.genre) LIKE lower(?)
			OR b.isbn LIKE ?
			OR lower(b.tags) LIKE lower(?)
		)`
		like := "%" + strings.TrimSpace(filters.Query) + "%"
		args = append(args, like, like, like, like, like)
	}
	if filters.Genre != "" && filters.Genre != "All" {
		where += " AND b.genre = ?"
		args = append(args, filters.Genre)
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
			&b.ID, &b.Title, &b.Author, &b.Genre, &b.Year, &b.ISBN, &b.CoverColor,
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

func (app *application) cartItems(r *http.Request) []cartItem {
	items, ok := app.sessions.Get(r.Context(), "cart").([]cartItem)
	if !ok {
		return nil
	}
	return items
}

func (app *application) cartView(r *http.Request) (cartView, error) {
	items := app.cartItems(r)
	view := cartView{
		ProgressText: "Your cart is empty. Add $35.00 more for free shipping.",
	}
	for _, item := range items {
		if item.Quantity <= 0 {
			continue
		}
		book, err := app.bookByCopyID(item.CopyID)
		if err != nil {
			if err == sql.ErrNoRows {
				continue
			}
			return cartView{}, err
		}
		if item.Quantity > book.Stock {
			item.Quantity = book.Stock
		}
		if item.Quantity <= 0 {
			continue
		}
		lineTotal := book.Price * float64(item.Quantity)
		view.Lines = append(view.Lines, cartLine{
			Book:      book,
			Quantity:  item.Quantity,
			LineTotal: lineTotal,
		})
		view.ItemCount += item.Quantity
		view.Subtotal += lineTotal
	}
	if view.Subtotal > 0 && view.Subtotal < 35 {
		view.Shipping = 4.95
	}
	view.Total = view.Subtotal + view.Shipping
	view.FreeShipping = view.Subtotal >= 35
	if view.Subtotal > 0 {
		remaining := 35 - view.Subtotal
		if remaining <= 0 {
			view.ProgressText = "Your stack qualifies for free shipping."
			view.ProgressRatio = 100
		} else {
			view.ProgressText = fmt.Sprintf("Add %s more for free shipping.", formatMoney(remaining))
			view.ProgressRatio = (view.Subtotal / 35) * 100
		}
	}
	return view, nil
}

func (app *application) bookByCopyID(copyID int64) (bookCard, error) {
	row := app.db.QueryRow(`
		SELECT
			b.id, b.title, b.author, b.genre, b.year, b.isbn, b.cover_color,
			b.aspect_ratio, b.tags, b.is_new_arrival,
			c.id, c.condition, c.price, c.notes, c.format, c.stock,
			c.is_staff_pick, COALESCE(c.staff_quote, ''), c.seal_style, c.seal_text
		FROM books b
		JOIN book_copies c ON c.book_id = b.id
		WHERE c.id = ? AND c.is_sold = 0`, copyID)
	var b bookCard
	err := row.Scan(
		&b.ID, &b.Title, &b.Author, &b.Genre, &b.Year, &b.ISBN, &b.CoverColor,
		&b.AspectRatio, &b.Tags, &b.IsNewArrival,
		&b.CopyID, &b.Condition, &b.Price, &b.Notes, &b.Format, &b.Stock,
		&b.IsStaffPick, &b.StaffQuote, &b.SealStyle, &b.SealText,
	)
	return b, err
}

func (app *application) bookByID(bookID string) (bookCard, error) {
	row := app.db.QueryRow(`
		SELECT
			b.id, b.title, b.author, b.genre, b.year, b.isbn, b.cover_color,
			b.aspect_ratio, b.tags, b.is_new_arrival,
			c.id, c.condition, c.price, c.notes, c.format, c.stock,
			c.is_staff_pick, COALESCE(c.staff_quote, ''), c.seal_style, c.seal_text
		FROM books b
		JOIN book_copies c ON c.book_id = b.id
		WHERE b.id = ? AND c.is_sold = 0
		ORDER BY c.is_staff_pick DESC, c.price ASC
		LIMIT 1`, bookID)
	var b bookCard
	err := row.Scan(
		&b.ID, &b.Title, &b.Author, &b.Genre, &b.Year, &b.ISBN, &b.CoverColor,
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
	schema, err := schemaFS.ReadFile("db/schema.sql")
	if err != nil {
		return err
	}
	_, err = db.Exec(string(schema))
	return err
}

func parseTemplates() (*template.Template, error) {
	funcs := template.FuncMap{
		"money":        formatMoney,
		"freePickup":   func(v float64) bool { return v > 8 },
		"listPrice":    func(v float64) float64 { return v * 1.5 },
		"priceDollars": func(v float64) int { return centsTotal(v) / 100 },
		"priceCents": func(v float64) string {
			return fmt.Sprintf("%02d", centsTotal(v)%100)
		},
		"selected": func(current, option string) bool { return current == option },
	}
	tmpl := template.New("").Funcs(funcs)
	var files []string
	err := filepath.WalkDir("templates", func(path string, d os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if !d.IsDir() && strings.HasSuffix(path, ".html") {
			files = append(files, path)
		}
		return nil
	})
	if err != nil {
		return nil, err
	}
	return tmpl.ParseFiles(files...)
}

func filtersFromRequest(r *http.Request) catalogFilters {
	q := r.URL.Query()
	return catalogFilters{
		Query:     strings.TrimSpace(q.Get("q")),
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

func formatMoney(v float64) string {
	return fmt.Sprintf("$%.2f", v)
}

func centsTotal(v float64) int {
	return int(math.Round(v * 100))
}

func quantityFor(items []cartItem, copyID int64) int {
	for _, item := range items {
		if item.CopyID == copyID {
			return item.Quantity
		}
	}
	return 0
}

func setCartQuantity(items []cartItem, copyID int64, quantity int) []cartItem {
	var out []cartItem
	found := false
	for _, item := range items {
		if item.CopyID != copyID {
			out = append(out, item)
			continue
		}
		found = true
		if quantity > 0 {
			item.Quantity = quantity
			out = append(out, item)
		}
	}
	if !found && quantity > 0 {
		out = append(out, cartItem{CopyID: copyID, Quantity: quantity})
	}
	return out
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
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
