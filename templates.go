package main

import (
	"fmt"
	"html/template"
	"math"
	"os"
	"path/filepath"
	"strings"
)

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

func formatMoney(v float64) string {
	return fmt.Sprintf("$%.2f", v)
}

func centsTotal(v float64) int {
	return int(math.Round(v * 100))
}
