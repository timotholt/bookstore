# Catalog Pagination Specification

Status: proposed implementation spec.

## Product goal

Search and catalog browsing must remain fast as Chantel's Corner grows beyond
10,000 books. A user must see a bounded result page, understand exactly which
results are visible, and move through the full filtered result set without
losing the search or filter state.

The first page should say, for example:

> Showing 1–24 of 10,000 results

For a filtered search, the total is the filtered total, not the size of the
whole catalog:

> Showing 49–72 of 318 results

## Core decision

Use server-side numbered pagination with a default page size of **24 books**.

Provide a standard `Items per page` select with fixed choices **24, 48, and
96**, in ascending order. Twenty-four is the default; 48 and 96 are bounded
power-user options. Do not expose an arbitrary numeric limit input.

Twenty-four fits the existing product grid at common desktop widths, keeps the
HTML and placeholder-cover work bounded, and avoids turning one catalog request
into a large page payload. The page size is a product setting, not a user-
controlled arbitrary SQL limit supplied by the browser.

The browser must never load all books and hide most of them with JavaScript or
CSS. HTMX enhances ordinary server-rendered links; it does not own pagination.

## URL contract

`GET /search` remains the canonical endpoint. Pagination state is represented
in the query string so pages are bookmarkable, shareable, crawlable, and
recoverable after refresh.

Supported parameters:

- `page`: one-based integer; absent means `1`.
- `per_page`: one of `24`, `48`, or `96`; absent means `24`.
- `sort`: existing sort values.
- all existing filters: `q`, `author`, `genre`, `condition`, `listing`,
  `max_price`, `format`, and `min_rating`.

Do not expose `offset` as the public contract. The handler derives it as
`(page - 1) * PAGE_SIZE` after validating and clamping the page.

Invalid values are safe and deterministic:

- missing, zero, negative, or non-numeric `page` becomes `1`;
- missing, zero, negative, non-numeric, or unsupported `per_page` becomes `24`;
- a page beyond the last page becomes the last valid page when results exist;
- pagination is `1` with no pagination controls when there are no results;
- unknown sort values fall back to the existing default sort.

Filter, sort, and page links must preserve every active query parameter except
the page value being changed. Changing a filter resets `page` to `1`.

## Backend design

### Catalog query object

Extend the catalog query contract so one call returns:

```text
CatalogPage {
    books: Vec<BookCard>,
    total_items: i64,
    page: u32,
    page_size: u32,
    total_pages: u32,
    first_item: Option<i64>,
    last_item: Option<i64>,
}
```

The request model gains deserialized `page` and `per_page` fields, or an
equivalent separate pagination request type. Normalize `per_page` against the
allowlist before it reaches SQL. Keep `result_text` as presentation output
only; do not make SQL or handler logic depend on formatted text.

### Required SQL behavior

The store layer builds the filtered relation once conceptually and exposes two
operations:

1. `COUNT(*)` over the filtered, de-duplicated book relation.
2. The same filtered relation with deterministic `ORDER BY`, `LIMIT per_page`,
   and `OFFSET`.

The current cheapest-unsold-copy rule must remain intact. The count must count
books/results after that rule, not raw `book_copies` rows. A book with several
available copies must not inflate the result count.

Every sort needs a unique final tie-breaker, such as `b.id`, after its existing
sort fields. This prevents books from moving between pages when titles, prices,
or years are equal during repeated requests.

The page query must never use an unbounded `fetch_all`. The only catalog result
query should have an explicit bounded limit.

### Count and facet separation

Remove the current pattern of loading the entire unfiltered catalog into
`all_books` only to calculate `"N of N items shown"` or derive filter options.
The result total comes from the filtered `COUNT(*)` query.

The filter option lists need their own bounded strategy: retain the existing
behavior for now only if it is already cheap and proven bounded; otherwise add
a distinct metadata query for genres, conditions, and formats. Facet queries
must not load 10,000 `BookCard` objects merely to populate selects.

The first implementation may run count and page queries sequentially for the
simplest error handling. If latency measurement shows this is material, run
the independent count and page queries concurrently without changing the
response contract.

### Database indexes

Before adding indexes, inspect the current migrations and query plan. The
implementation should verify indexes for:

- unsold-copy filtering and book/copy joins;
- book-to-author and book-to-genre joins;
- fields used by active filters;
- the search representation used by `search_text`.

If `%term%` matching remains the dominant search cost, a follow-up migration
may add PostgreSQL full-text or trigram support. That is a separate search
optimization and must not be mixed into the pagination change without an
`EXPLAIN (ANALYZE, BUFFERS)` comparison.

## Response and template contract

`CatalogResultsTemplate` receives a `CatalogPage`/pagination view object in
addition to the product cards. The reusable component renders:

1. result summary;
2. sort control;
3. `Items per page` select;
4. bounded product grid;
5. empty state when `total_items == 0`;
6. pagination navigation when `total_pages > 1`.

The summary uses inclusive one-based bounds:

- `first_item = ((page - 1) * page_size) + 1`;
- `last_item = min(page * page_size, total_items)`.

Use `Showing {first_item}–{last_item} of {total_items} results`. Use `result`
for a total of one and `results` otherwise. Do not say “items shown” when the
count is a total matching set; that wording is ambiguous after pagination.

The page-size control uses a native `<select>` labeled `Items per page`.
Changing it preserves all active filters and sort, resets `page` to `1`, and
submits through the existing HTMX form behavior. No separate Go button is
needed. The selected value must survive full-page loads and HTMX swaps.

## Pagination controls

Render semantic navigation:

```html
<nav aria-label="Search results pages">
  <a rel="prev">Previous</a>
  <a aria-current="page">1</a>
  <a>2</a>
  <span aria-hidden="true">…</span>
  <a>Last</a>
  <a rel="next">Next</a>
</nav>
```

Rules:

- hide Previous on page 1;
- hide Next on the last page;
- show the current page with `aria-current="page"`, not as a live link;
- show a compact window around the current page plus first and last pages;
- omit ellipses when the neighboring range is contiguous;
- use real anchors with complete query-string URLs;
- provide descriptive accessible labels, e.g. `Go to page 3`;
- keep keyboard focus visible and do not trap focus in an HTMX swap.

Pagination belongs below the grid. On HTMX navigation, replace the complete
`#catalogResults` region so the summary, grid, and controls cannot disagree.

## HTMX behavior

Pagination links are progressively enhanced:

- normal browser behavior follows the anchor and renders `/search?...`;
- HTMX links use `hx-get` with `hx-target="#catalogResults"`,
  `hx-swap="outerHTML"`, and `hx-push-url="true"`;
- the filter form continues to target the same results region and resets page
  to `1`;
- changing `per_page` targets the same results region, resets `page` to `1`,
  and pushes the resulting URL;
- the response for an HTMX request is the results partial only;
- the response for a normal request is the full search page;
- loading state uses the existing UI conventions and must not leave stale
  cards visible as if they belong to the new page.

After an HTMX swap, focus should move to the results heading/summary or remain
on the activated pagination control according to the existing accessibility
convention. Add a small, explicit focus behavior only if current scripts do
not already provide one.

## Sort and consistency rules

Sort changes reset to page 1. The selected sort remains in the URL and in the
select control after both full-page and HTMX requests.

Changing `per_page` also resets to page 1. `per_page` remains in every page,
Previous, Next, and numbered-page URL so navigation does not silently revert to
24.

Use stable ordering for every sort:

- popularity/default: existing popularity fields, then title, then `b.id`;
- price ascending/descending: price, then title, then `b.id`;
- year descending: year, then title, then `b.id`.

This is offset pagination, so inserts or sales between requests can still move
items across pages. That is acceptable for this storefront phase. If catalog
mutation becomes frequent enough to make this visible, the next redesign is a
cursor/keyset contract, not client-side caching.

## Verification gates

### Unit and integration tests

Add tests proving:

- default request returns page 1 with at most 24 books;
- `per_page=48` and `per_page=96` return bounded pages of the selected size;
- unsupported `per_page` values normalize to 24;
- page 2 returns the next slice and preserves filters;
- summary text is correct for first, middle, and final pages;
- a filtered total is not the unfiltered catalog total;
- duplicate copies do not inflate `total_items`;
- invalid and out-of-range pages are safe;
- sort and filters reset page to 1;
- changing page size resets page to 1 and preserves filters/sort;
- query-string links preserve all active filters;
- one-result and zero-result wording/control states are correct.

### Route smoke checks

With PostgreSQL configured, verify:

```text
GET /search
GET /search?page=2
GET /search?per_page=48
GET /search?per_page=96&page=2
GET /search?q=history&page=2
GET /search?genre=Fiction&sort=price-asc&page=3
GET /search?page=999999
GET /search?q=does-not-exist
GET /search?... with HX-Request: true
```

For each response, inspect status, result summary, number of cards, selected
filters, pagination URLs, and the absence of unbounded catalog loading.

### Performance proof

Capture before/after measurements on the same database:

- response time for unfiltered page 1;
- response time for a filtered page;
- response body size;
- database query count;
- `EXPLAIN (ANALYZE, BUFFERS)` for count and page queries.

Acceptance requires that the page query is bounded at the selected allowlisted
size (24, 48, or 96) and that the
application no longer materializes the full 10,000-book catalog for a search
request. Exact latency targets should be set after the first measurement,
because count cost and search-index cost may differ from page-fetch cost.

## Explicit non-goals

- no JavaScript-only pagination;
- no infinite scroll in this phase;
- no arbitrary `per_page` values outside the allowlist;
- no cursor pagination until offset behavior is measured and shown inadequate;
- no search-index migration bundled into the first pagination patch unless the
  query plan proves pagination alone cannot meet the measured goal.

## Implementation sequence

1. Add page parsing and a pagination view object with unit tests.
2. Refactor the store into filtered count plus bounded page query.
3. Add deterministic tie-breakers and verify duplicate-copy semantics.
4. Update full-page and HTMX catalog responses to use the same page contract.
5. Add the summary and accessible pagination component/styles.
6. Add route, integration, and query-plan/performance checks.
7. Only then evaluate indexes or cursor pagination using measured evidence.
