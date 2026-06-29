package main

import (
	"database/sql"
	"encoding/gob"
	"html/template"

	"github.com/alexedwards/scs/v2"
)

type application struct {
	db        *sql.DB
	sessions  *scs.SessionManager
	templates *template.Template
}

type bookCard struct {
	ID           string
	Title        string
	Author       string
	AuthorSlug   string
	Genre        string
	GenreSlug    string
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

type variantAttribute struct {
	Name  string `json:"Name"`
	Value string `json:"Value"`
}

type bookDetailPageData struct {
	Title      string
	Genres     []string
	Book       bookCard
	Copies     []bookCard
	Attributes map[int64][]variantAttribute
	Related    []bookCard
	Cart       cartView
}

type catalogFilters struct {
	Query      string
	Author     string
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
