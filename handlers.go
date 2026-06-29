package main

import (
	"database/sql"
	"fmt"
	"net/http"
	"strconv"
	"strings"

	"github.com/go-chi/chi/v5"
)

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
	bestSellers, err := app.collectionBooks("best-sellers", 6)
	if err != nil {
		http.Error(w, "best sellers unavailable", http.StatusInternalServerError)
		return
	}
	deals, err := app.collectionBooks("used-deals", 6)
	if err != nil {
		http.Error(w, "deals unavailable", http.StatusInternalServerError)
		return
	}
	staffPicks, err := app.collectionBooks("staff-picks", 3)
	if err != nil {
		http.Error(w, "staff picks unavailable", http.StatusInternalServerError)
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
		BestSellers:  bestSellers,
		NewArrivals:  firstWhere(allBooks, 6, func(b bookCard) bool { return b.IsNewArrival }),
		Deals:        deals,
		Catalog:      books,
		StaffPicks:   staffPicks,
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
	currentURL := r.Header.Get("HX-Current-URL")
	if strings.Contains(currentURL, "/cart") {
		w.Header().Set("HX-Redirect", "/cart")
		w.WriteHeader(http.StatusNoContent)
		return
	}

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

type cartPageData struct {
	Title  string
	Genres []string
	Cart   cartView
}

func (app *application) cartPage(w http.ResponseWriter, r *http.Request) {
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
	data := cartPageData{
		Title:  "Shopping Cart | Davis's Books",
		Genres: uniqueGenres(allBooks),
		Cart:   cart,
	}
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	if err := app.templates.ExecuteTemplate(w, "cart-page", data); err != nil {
		http.Error(w, "template error", http.StatusInternalServerError)
	}
}
