package main

import (
	"log"
	"net/http"
	"os"
	"time"

	"github.com/alexedwards/scs/v2"
)

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
