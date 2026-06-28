package main

import (
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
)

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
