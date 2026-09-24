# Database read budgets and Railway-local catalog cache

Status: implemented for verification on September 24, 2026. Release evidence below distinguishes local checks from deployment.

## Problem and decisions

The previous default catalog query had no LIMIT. Homepage, detail, cart, and account chrome downloaded the full catalog to select a few books or extract genre names. These were confirmed over-fetching paths, not a measured attribution of all historical Neon transfer.

Use **Moka 0.12 with its `future` feature**, inside the Rust Railway process. No cache server, persistent volume, new database, or full-catalog preload. PostgreSQL remains authoritative. A deployment starts an empty cache and loads only requested bounded slices.

Keep SQL in `store.rs`, reuse BookCard and existing Askama includes, and share normalized predicates between catalog results and their count. The cache is scoped to the router, shared by its requests and clones, and separated between router instances/tests. Tokio task-local request context carries the cache, row budget, and secondary pagination without putting account data into cache keys.

## Hard read boundaries

- Maximum **100 rows per SQL query**. Every runtime multirow read has a SQL LIMIT and calls `bounded_rows(max)`; singleton and aggregate reads call `bounded_one()`.
- Maximum **200 result rows per request**, with two conservatively reserved for the session library's singleton reads. Each query reserves its maximum before its SQL future is polled and refunds unused rows after success. A failure keeps its reservation. A budget rejection starts no query.
- Email worker iterations receive their own bounded operation scope. Startup migrations and administrative tools are outside HTTP request accounting; they must not be used as catalog export endpoints.
- Batch copy IDs are deduplicated and rejected above 100. No automatic chunk loop. Default catalog size is 24; UI choices remain 24/48/96. Repository calls reject page sizes outside 1–100.
- No post-fetch truncation, unlimited streams, or aggregated arrays hiding an entire catalog in one row. Scalar COUNT/EXISTS is allowed.
- These rules limit rows **transferred to Rust**, not rows scanned internally by PostgreSQL. Aggregate counts can inspect the full catalog and return one number.
- A source-level regression test rejects runtime fetches without the budget adapter and rejects new streaming/raw SQL in the audited modules. Large-catalog integration tests prove SQL-side bounds, not merely a post-fetch assertion. Future queries still require reviewing their SQL LIMIT; the adapter cannot rewrite arbitrary SQL.

Budget exhaustion and the catalog fill-rate limit produce HTTP 503 with Retry-After. There is no unbounded fallback. Direct administrative PostgreSQL clients are not governed by this server's limits.

## Query and response shapes

| Consumer | Read shape |
| --- | --- |
| Homepage | One featured record/fallback; shelves of 6, 6, 3, 4, and 6 |
| Navigation | Up to 24 genre names; full genre options are paginated on Search |
| Catalog/search | 24/48/96 compact records plus a scalar total |
| Related books | Genre and exclusion predicates in SQL, LIMIT 4 |
| Detail | One book; 10 copies per page; 50 attributes per page for the displayed copies |
| Facets | 24 genres, 8 conditions, 12 formats per options page; scalar totals provide continuation |
| Cart and saved items | 20 lines per page, with copy lookup only for those IDs |
| Cart totals | SQL aggregates over all available items, regardless of displayed page |
| Checkout preview | 20 displayed lines with continuation and complete aggregate totals |

All pagination has deterministic tie-breakers. Secondary navigation uses one Rust PageNavView, shared Askama include, reusable links, and a `.ui-page-navigation` class. Navigation preserves query parameters. Oversized baskets are paginated rather than truncated or given a new total-item restriction. Out-of-stock/deleted lines do not inflate aggregate totals.

List/card SQL does not transfer book descriptions or tags. Detail descriptions are queried separately. Selected text fields have SQL-side size bounds (including title/author 512 characters, cover URL 2,048, notes 2,048, detail description 32,768); stored data is unchanged. Large descriptions beyond the display cap are not downloaded by this UI.

## Moka policy

| Cache partition | Accounted capacity | Expiration |
| --- | --- | --- |
| Offer-bearing book cards, shelves, copy pages, detail counts | 32 MiB | 30 seconds |
| Catalog totals | 8 MiB | 5 minutes |
| Descriptions, genres, facet options/counts, variant attributes | 24 MiB | 1 hour |

Keys include query kind, normalized filters/ID, relevant page, and catalog generation. Counts exclude irrelevant sort/page values. Filter values are bounded to 256 bytes. Cache keys are bounded to 4 KiB.

Values are serialized public data held in Arc; weights account for key bytes, serialized bytes, and 1 KiB per-entry overhead. Moka's eviction is best-effort; 64 MiB is the accounted cache capacity, not a hard total process-RAM guarantee. Allow headroom for allocator/index/in-flight data.

`try_get_with` coalesces concurrent misses for the same key. At most two fill operations run concurrently, with a two-second admission wait. A process-wide fixed-window limit permits at most 120 fills per minute; cache hits do not spend this allowance. This caps uncontrolled search/cache churn but is not a monthly billing guarantee. No background preload, full scan, or eager refresh is performed.

Failures are never cached as successful data or as nonexistent books. This release uses expiration and controlled failure, without stale-on-error serving: avoiding stale purchasable offers is preferable to adding a second stale-cache mechanism before it is needed. A warm valid entry is usable while PostgreSQL is unavailable; a cold/expired entry cannot manufacture data.

Book cards include price/availability, so their complete snapshots expire after 30 seconds. Descriptions and dimensions can live longer. This deliberately keeps the first implementation simple rather than introducing per-ID metadata hydration that would double cold-page queries. Measure actual transfer before adding another cache layer.

## Freshness and personal state

- Account, session, cart, saved-item, checkout totals, stock validation, and email operations are not globally cached.
- Cart mutations and rendering read current copy stock/prices. Cached catalog display values never authorize stock or price decisions.
- Successful non-analytics POST requests invalidate this process's catalog caches. This is conservative and can later be narrowed when dedicated inventory mutation handlers exist.
- Invalidation increments a generation before clearing entries. Late in-flight fills retain their old generation and cannot populate a new generation's key.
- Direct imports/administrative SQL must be followed by a controlled Railway restart/redeploy for immediate freshness. Otherwise TTL bounds staleness. Do not assume out-of-process writes automatically notify Moka.
- Each replica has a separate cache. Expiration applies per replica; this release has no cross-replica invalidation bus. Before introducing catalog writes across multiple replicas, add a broadcast invalidation mechanism or explicitly accept TTL-bounded display staleness.

## Observability and cost

Existing tracing records request database-row accounting and cache fill name/payload bytes. Debug logs provide cache hits and individual read maxima. Values, credentials, raw queries, personal data, and search terms are not logged. Serialized payload bytes are estimates, not exact PostgreSQL wire accounting.

Use Neon project transfer metrics for the actual monthly total. Row caps alone cannot cap the bill: other clients, administration, and repeated legitimate requests also use transfer. Set provider alerts against the owner's chosen budget; no automatic paid upgrade is part of this release.

## Verification

Use disposable local PostgreSQL, never production, for schema-dropping tests. The test suite adds:

- A 10,001-book fixture proving default reads return 24 records, totals exceed 10,000 correctly, oversized sizes/ID batches are rejected, and cold HTTP routes stay within 200 rows.
- Repeated anonymous routes proving no application PostgreSQL queries after warming (the reported two rows are the conservative session allowance).
- A 45-line cart/saved-items fixture proving all three pages are accessible and show the full $225 subtotal; a live price update changes the cart to $270 despite cached catalog snapshots.
- Copy/attribute pagination, shared cache fill coalescing, failure retry, generation invalidation during an in-flight fill, and explicit runtime read guards.
- Existing authentication, account email, cart/HTMX, stock, search, and template tests.

Measured on the 10,001-book fixture, fresh router per route (includes two reserved session rows): homepage 39 cold / 2 warm; 96-card search 124 / 2; detail 21 / 2; cart and login 13 / 2. These are local fixture measurements, not production traffic forecasts.

Deployment requires formatting, compilation, lint, tests, browser smoke checks, a verified GitHub release, and matching live `/version`, `/readyz`, homepage, catalog, detail, and cart responses. A restored Neon plan is separate from deployment of these safeguards.

## Neon budget monitoring

The server polls Neon's v2 consumption history API every five minutes, independently of page requests and without waking Postgres. `NEON_API_KEY` and `NEON_ORG_ID` enable organization-wide database usage estimates; `OPERATIONS_METRICS_TOKEN` (at least 32 characters) protects `GET /ops/usage`. Credentials belong only in Railway variables and ignored local secret files. The homepage uses the in-memory snapshot and shows an alert above $0.50 for the current UTC calendar day, a warning at $8/month, and an alert at $10/month. These alerts do not suspend the service or enforce a provider spending cap.

Monthly estimates cover the current UTC calendar month across all returned projects and branches. Supported rate cards are Launch and Scale, verified on 2026-09-24. Compute is CU-seconds / 3600; v2 storage is byte-months / 1e9 (not byte-hours), with $0.35 storage, $0.20 instant restore, and $0.09 snapshot rates. Public transfer includes each project's 500 GB/month allowance, applying only incremental overage to today's cost. Branch charges subtract the plan's included child branch-hours per bucket, then divide by 744. Daily aggregation can differ slightly from hourly invoices. Taxes, credits, negotiated rates, non-database Neon products, and Railway charges are excluded. These are estimates from delayed metering, never a guaranteed final bill.

Missing configuration, errors, unknown plans/metrics, incomplete pagination, absent current-day data, or a snapshot older than 15 minutes produce an unavailable notice rather than zero usage. Failed polls retain the last successful snapshot. No provider calls occur on page refresh. A process restart refetches provider history; PostgreSQL is not used to store monitoring data. Cache/query counters are explicitly per-process and reset on restart. Cache-fill byte counts describe serialized application payload, not exact Neon wire transfer or billable bytes.

Sources: [Neon consumption API](https://neon.com/docs/reference/api/consumption/get-consumption-history-per-project-v2), [cost calculation formulas](https://neon.com/docs/introduction/usage-calculations), [plan pricing](https://neon.com/docs/introduction/plans). Recheck the rate card when changing plans or when Neon changes prices. Budget checks are operational warnings; they cannot guarantee a $10 maximum bill.

### Local release checks

On 2026-09-24: workspace check and warning-free clippy passed; 56 application tests and 21 xtask tests passed in the full run. The added refresh/protected-metrics integration test passed separately, and the explicitly ignored email outbox test passed against the same disposable local database. Chrome verified search pagination, book detail, and Add to Stack updating the cart and totals. The real Neon v2 poll returned a successful organization usage estimate without a database query. Deployment remains to be verified.
