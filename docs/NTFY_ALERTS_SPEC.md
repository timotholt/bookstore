# Phone Alerts (ntfy) Spec

Status: proposed, unbuilt (2026-10-16).

## Goal

Push a short phone notification to the owner when something needs attention or costs money: Neon spend, a review waiting for moderation, a failed email, a staff/security change, a deploy or crash restart. Nothing else. The phone must stay quiet on a normal day.

## Non-goals

- No customer personal data in any push: no emails, names, user ids, addresses, IP addresses, review text or report reasons. ntfy topics on `ntfy.sh` are readable by anyone who knows the topic name, and pushes pass through Apple/Google push services and the ntfy cache. Details stay in the database, admin pages and server logs; a push only says what happened and where to look.
- No per-event pushes for logins, logouts, cart adds/removes or page views. They are noise and carry no action.
- No new database polling. Neon scale-to-zero is the main cost control (`DATABASE_READ_BUDGET_AND_CACHE_SPEC.md`, "Idle-cost audit"); alerts must never wake the compute on their own.
- No order alerts yet. Ordering is closed; "order placed" is added when a real order/payment workflow exists.

## Events

| Event | Trigger point | Priority | Collapse rule | Body (example) |
| --- | --- | --- | --- | --- |
| Neon spend exceeded | `UsageMonitor::refresh` (`src/usage.rs`), when `Snapshot::notice` moves to level `error` | 5 (urgent) | Once per UTC day while exceeded | "Neon $0.62 today (alert $0.50), $7.10 this month (budget $10.00)" |
| Neon budget warning | Same, when notice moves to level `warning` with a cost (≥ 80% of month) | 4 | Once per UTC month | "Neon at 82% of monthly budget ($8.20 / $10.00)" |
| Usage monitoring down | Same, when the "Usage monitoring unavailable" notice has lasted ≥ 30 minutes | 3 | Once until it recovers; one "recovered" push | "Neon usage checks failing since 14:05 UTC" |
| Read budget refusing requests | Same refresh: `BUDGET_REJECTIONS` grew since the last refresh | 4 | Once per hour | "Read budget refused 12 requests in the last 5 min" |
| Review waiting for moderation | `reviews::submit` after commit (new submission or edit → pending) | 3 | At most one push per 30 min; body carries the count since the last push | "2 reviews waiting · /staff/reviews" |
| Review reported | `reviews::report` after commit | 3 | Same 30-min collapse, own key | "1 new review report · /staff/reviews" |
| Email permanently failed | Email worker, when a message is finished with status `failed` (`src/email/mod.rs`) | 4 | At most one per hour; count + error category | "3 emails failed (provider_rejected)" |
| Staff / account security change | `admin::action` and operator commands, after the `admin_audit` insert commits | 4 | None (rare) | "Staff role granted: moderator" / "Account disabled" |
| New signup | `auth::register_user` after commit | 2 (silent) | At most one push per 6 h with the count | "4 new accounts" |
| Server started | `main`, after the listener binds | 2 | None | "Started c3a6c3c (deploy 1a2b…)" from `RAILWAY_GIT_COMMIT_SHA` / `RAILWAY_DEPLOYMENT_ID` |

"Server started" covers deploys and crash restarts. A second start without a matching deploy id change is a crash loop; the body says "restart, same deploy" in that case (previous id kept in memory only, so the first start after a deploy is always reported as a deploy).

Railway web server cost is not metered here; the Neon alerts are the spend signal this repo can see.

## Design

New module `src/notify.rs`, one job: send short alerts to ntfy without ever slowing or failing a request.

- `Notifier::from_env()` reads `NTFY_URL` (default `https://ntfy.sh`), `NTFY_TOPIC` and optional `NTFY_TOKEN`. Missing topic → disabled; every call is a no-op. Local runs and tests are disabled by default.
- `Notifier::send(Alert { key, title, body, priority, tags, click })`: non-blocking. Pushes onto a bounded `tokio::sync::mpsc` channel (capacity 64); a full channel drops the alert and logs a `tracing::warn!`. Never returns an error to the caller.
- One background task drains the channel and POSTs to `{NTFY_URL}/{NTFY_TOPIC}` with headers `Title`, `Priority`, `Tags`, optional `Click` (a site path such as `/staff/reviews` joined to `PUBLIC_BASE_URL`) and `Authorization: Bearer` when a token is set. Reuses `reqwest` (already a dependency): 5 s timeout, no redirects, one retry, then drop and log.
- Collapse/cooldown lives in the notifier, in memory, keyed by `Alert::key`: within the window, alerts increment a counter; the next allowed push carries the count. State resets on restart; that is acceptable (worst case one extra push after a deploy).
- `Notifier` lives in `AppState` beside `UsageMonitor` and `EmailService`.
- Alerts fire only after the owning transaction commits, so a rolled-back action never pushes.
- No database access, no new timers. The notifier task sleeps on its channel; the 5-minute Neon usage refresh is the only periodic driver and already exists (control-plane HTTP, no database).

## Topic and secrets

- `NTFY_TOPIC` and `NTFY_TOKEN` are Railway variables, never committed. Add their names (fake values) to the env example per `INFRASTRUCTURE_SPEC.md`.
- On public `ntfy.sh`, the topic name is the only protection: use a random topic of at least 32 characters. A reserved topic with an access token (paid ntfy.sh or self-hosted ntfy) is better and needs only `NTFY_TOKEN` set; no code change.
- The phone subscribes to that one topic in the ntfy app.

## Tests

- `notify.rs` unit tests with a test sink in place of HTTP: disabled when unconfigured; collapse window counts and carries the count; full channel drops without blocking; bodies built for each event contain no email address or user id (assert on the built `Alert`).
- One test per hook that the alert is emitted after commit and not on a failed/rolled-back action (review submit and admin action are enough; the rest share the helper).
- `Snapshot::notice` transitions (ok → warning → error, unavailable ≥ 30 min → recovered) produce exactly one alert each, using `set_cost_for_test`.
- No test reaches the network.

## Build order

1. `notify.rs` + `AppState` wiring + "Server started" + Neon spend/warning/monitoring alerts. This is the spend tracking and can ship alone.
2. Review pending/reported, email failed, staff/security changes.
3. New signup count.

Each step: `cargo check`, `cargo test`, then a local smoke run with a throwaway topic to see a real push on the phone.

## Open questions for the owner

- Daily summary push (signups, reviews, emails, Neon spend for the day)? It would need in-memory counters sent on the first usage refresh after a set hour; not in this spec unless wanted.
- Failed-login spikes: worth an alert (count only, no account or IP) or leave to logs?
