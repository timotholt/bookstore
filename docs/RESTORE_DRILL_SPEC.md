# Backup Restore Drill Spec

Status: proposed 2026-11-17. Not run.

## Problem

Production Postgres has had Railway point-in-time recovery (PITR) enabled since the 2026-11-04 cutover, and the archive looks healthy:

- `pgbackrest info` (stanza `main`) reports status `ok`, with 14 backups in the repository.
- Full backups run weekly (the latest is `20261111-100716F`) and differential backups daily around 10:07 UTC (the latest is `20261117-101112D`). The oldest full is from 2026-11-04 10:06 UTC.
- WAL is pushed asynchronously, compressed with zstd, to the `Postgres-PITR` Railway bucket. `pg_stat_archiver` shows 19,001 archived segments and 0 failures.

Nobody has restored from it. Until a restore has worked end to end, we don't actually know we have backups. `INFRASTRUCTURE_SPEC.md` lists this as its first open item.

## How Railway restores work

Per [Railway's PITR docs](https://docs.railway.com/volumes/point-in-time-recovery):

- A restore never happens in place. It creates a **new sibling service** (default name `<source>-restored-YYYYMMDD-HHMM`). On first boot that service runs `pgbackrest restore --type=time --target=<T>` from the source's archive.
- The source service is not touched and keeps serving.
- The fork copies the source's variables. Its archive credentials are carried under a recovery prefix and have **read/write access to the source archive**; there is no read-only credential. The fork itself runs as plain Postgres that does not archive.
- Restore traffic is free. The fork is billed as a normal service (compute plus volume) for as long as it exists.

CLI (installed `railway` 5.71.0):

```bash
railway postgres pitr status --service Postgres
railway postgres pitr restore --service Postgres --at <RFC3339> --new-service-name <name> --yes
railway service delete --service <name> --environment production --yes
```

**`railway delete` deletes the whole project. Never use it in this drill.** Only `railway service delete`, with the fork's exact name.

## Goals

1. Restore production to a chosen recent time into a scratch service, and show the data matches the source as of that time.
2. Prove WAL replay works past the last base backup, not just that a base backup unpacks.
3. Measure recovery time: from issuing the restore to the fork accepting queries.
4. Leave production exactly as it was, and delete the fork the same day.
5. Write the working procedure and measured times into `INFRASTRUCTURE_SPEC.md`.

## Non-goals

- Cutting the web service over to the fork. That is a separate, riskier drill; the procedure below records what a real cutover would need.
- Testing Railway volume snapshots (`pitr backup create`) or a manual `pg_dump`. This drill is about PITR.
- Changing retention or schedules.

## Safety rules

- Use read-only queries against production. The only production action is the restore command, which reads the archive.
- The fork gets no public TCP proxy and no domain, and no service's `DATABASE_URL` points at it.
- Before deleting, compare the fork's service ID with the source `Postgres` ID and the `web` ID, and confirm the name ends with `-restore-drill-…`.
- Delete the fork as soon as verification is done: it holds copies of production data and the archive's write credential.
- Abort, and delete the fork, if the restore output mentions modifying `Postgres` or `web`, if the fork tries to archive WAL, or if anything targets the original volume.

## Procedure

Run by the operator from the Mac, with the Railway CLI logged in. Times are UTC.

### 1. Preflight (read-only)

- `railway postgres pitr status --service Postgres`: should show enabled, with the bucket wired.
- `pgbackrest info` inside the Postgres container, via `railway ssh --service Postgres`: stanza `ok`, and the latest backup is from today.
- Record the service IDs for `Postgres` and `web` from `railway status --json`.
- Choose a target time **T** at least 5 minutes in the past, and after 2026-11-17 14:40, when migration `20261117020000` (the `expires_at` index) was applied. A target after that migration but after today's 10:11 differential backup means the restore must replay WAL to show the index.

### 2. Fingerprint the source as of T

Rows that changed after T would make a naive comparison fail. So compare only tables that are stable on a quiet demo, and filter time-stamped rows where possible.

Run these against production as `postgres`, inside a `BEGIN TRANSACTION READ ONLY`:

```sql
SELECT max(version) AS latest_migration, count(*) AS migrations FROM _sqlx_migrations WHERE success;
SELECT 'books' AS t, count(*), md5(string_agg(id, ',' ORDER BY id)) FROM books
UNION ALL SELECT 'book_copies', count(*), md5(string_agg(id::text || ':' || stock::text || ':' || price::text, ',' ORDER BY id)) FROM book_copies
UNION ALL SELECT 'authors', count(*), md5(string_agg(id::text, ',' ORDER BY id)) FROM authors
UNION ALL SELECT 'genres', count(*), md5(string_agg(id::text, ',' ORDER BY id)) FROM genres
UNION ALL SELECT 'users_before_T', count(*), md5(string_agg(id::text, ',' ORDER BY id)) FROM users WHERE created_at <= :'T'
UNION ALL SELECT 'reviews_before_T', count(*), md5(string_agg(id::text, ',' ORDER BY id)) FROM reviews WHERE created_at <= :'T';
SELECT indexname FROM pg_indexes WHERE indexname = 'account_email_rate_limits_expires_at_idx';
```

Column names (`stock`, `price`, `created_at`) are checked against the schema before running, and adjusted to whatever the schema has. Keep the output in the scratchpad, not the repo.

### 3. Restore

```bash
railway postgres pitr restore --service Postgres --at <T> --new-service-name postgres-restore-drill-20261117 --yes --json
```

Record the time the command was issued (**t0**). Watch the fork's deploy logs: `railway logs --service postgres-restore-drill-20261117`. They should show pgBackRest restoring, recovery replaying WAL, `recovery stopping before … time T`, and the database becoming ready. Record when it first accepts a query (**t1**).

### 4. Verify the fork

Through `railway ssh --service postgres-restore-drill-20261117 -- psql -U postgres -d chantels_corner_demo`:

1. `SELECT pg_is_in_recovery();` returns `f`: the fork has finished recovery and is writable.
2. `SHOW archive_mode;` is `off`, or the archive command is unset: the fork does not push WAL.
3. The step 2 fingerprints match exactly.
4. The index `account_email_rate_limits_expires_at_idx` exists, proving WAL replay past the 10:11 differential backup.
5. `SELECT count(*) FROM pg_tables WHERE schemaname = 'public';` returns 37. Both `chantels_storefront` and `postgres` roles exist.

Then confirm the source is unaffected:

- The web service's `/version` and `/readyz` are unchanged.
- `pgbackrest info` on the source still shows status `ok`.
- `pg_stat_archiver.failed_count` is still 0.
- The `web` service's `DATABASE_URL` still points at `postgres.railway.internal`.

### 5. Teardown

- Compare IDs (see Safety rules), then run `railway service delete --service postgres-restore-drill-20261117 --environment production --yes`.
- Confirm in `railway status` or on the dashboard that the fork and its volume are gone, and that `Postgres` and `web` remain.

### 6. Record

Update `INFRASTRUCTURE_SPEC.md`:

- Replace "Rebuild from scratch" step 2 with the verified restore procedure.
- Record the measured recovery time (t1 − t0) and the date.
- Close the "Backup restore is untested" open item, or record what failed.
- Mark this spec implemented, with the outcome.

## Cost

The fork runs a Postgres instance with a volume about the size of the source (around 300 MB) for an estimated 15–30 minutes. Restore traffic from the bucket is free. Expect a few cents. It is still a cost-incurring operation, so it needs an explicit go-ahead.

## What a real cutover would add (not drilled)

If production ever has to move to a fork:

- Roles, passwords, grants and default privileges live in the database catalog, so they come back with the restore. Check that the web `DATABASE_URL` credentials log in to the fork.
- Re-enable PITR on the fork.
- Point the `web` service's `DATABASE_URL` at the fork's private domain, then redeploy. `/readyz` and the verify-mode migration check confirm it.
- Retire the old service only after the new one has archived its first base backup.
