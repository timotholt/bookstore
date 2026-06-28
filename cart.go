package main

import (
	"database/sql"
	"net/http"
)

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
			view.ProgressText = "Add " + formatMoney(remaining) + " more for free shipping."
			view.ProgressRatio = (view.Subtotal / 35) * 100
		}
	}
	return view, nil
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
