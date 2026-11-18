# Backup Restore Drill Spec

Status: run 2026-11-17 (**failed**) and re-run 2026-11-18 (**one of two targets restored**); see Results and Re-run results. Production was not affected either time.

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

## Results (2026-11-17)

**Outcome:** the restore failed. The fork never reached the target time, so the backups are **not proven restorable** for a target 9 hours after the last base backup.

| Step | Result |
| --- | --- |
| Preflight | PITR enabled; stanza `ok`; 14 backups; latest differential `20261117-101112D` (10:11 UTC); archiver had 0 failures |
| Target T | `2026-11-17T19:18:00Z` |
| Source fingerprint | Latest migration `20261117020000` (23 migrations); books 30,042, book_copies 30,044, authors 15,723, genres 25; users created by T: 2; reviews created by T: 1; 37 tables; roles `chantels_storefront` and `postgres` |
| Restore issued | 19:24:03 UTC. The workflow created `postgres-restore-drill-20261117` with its own 5,000 MB volume (the same size as the source). |
| Fork | Unpacked the 10:11 base backup and replayed WAL to about LSN `49/5A`, the last transaction at **14:35:32 UTC**. pgBackRest `archive-get` then failed with `[28] No space left on device` writing `pg_wal/RECOVERYXLOG`, and Postgres stopped: `FATAL: recovery ended before configured recovery target was reached`. It never accepted connections, so recovery time could not be measured and the fingerprints could not be compared. |
| Production | Unaffected. `/readyz` ready; web `DATABASE_URL` still `postgres.railway.internal`; archiver `failed_count` 0; stanza `ok`. |
| Teardown | Deleted the fork service `7efaae5b-…` after an ID check. Its volume `postgres-restored` (`bda488ee-…`) was left detached and was deleted separately (pending deletion at 19:40 UTC). The `Postgres` volume `881d24e0-…` was untouched. |

### What is known

- Archiving forces a WAL segment switch every minute (`archive_timeout = 60`), and Railway's idle heartbeat makes sure there is WAL to switch. So the database uses one 16 MB segment of WAL address space per minute even when idle: LSN `4A/43` (about 297 GB of address space) since 2026-11-04, roughly 23 GB a day. Compressed in the bucket these segments are tiny, but restore writes each one back at full size.
- The fork replayed about 4.4 hours of WAL (10:11 → 14:35, roughly 4.5 GB at 16 MB a minute) before its 5 GB volume filled. Replaying to 19:18 would have needed about 9 hours of WAL.
- The source does checkpoint every 5 minutes (3,869 checkpoints since 2026-11-04), so the WAL is not missing checkpoint records. **Why** the fork kept replayed WAL on disk instead of recycling it at restartpoints was not established. The likely suspects are restartpoint timing during fast replay, or the pgBackRest async `archive-get` spool (`archive-get-queue-max=1GiB`) sharing the volume. Railway's restore image is not ours to change.

### Current exposure

Until this is fixed, assume a restore can only reach a few hours past the most recent base backup. Base backups run daily around 10:07 UTC, so the evenings are the least covered.

### Options (not yet chosen)

1. **More frequent base backups:** a differential every few hours, so that no target needs more than a few GB of WAL replay. Chosen; planned in [BACKUP_SCHEDULE_SPEC.md](BACKUP_SCHEDULE_SPEC.md). (`railway postgres pitr schedule set` manages volume snapshots, not these backups.)
2. **Larger volume:** grow the `Postgres` volume, if the Railway plan allows it; the fork inherits that size. This costs storage around the clock for a problem that only appears during restores.
3. **Less WAL:** set `WAL_HEARTBEAT_DISABLED=1` on `Postgres`. An idle database then produces little WAL, but the worst-case data loss on an idle database grows from about 1 minute to about 5. Railway-specific; check it with Railway first.
4. **Report to Railway:** the default template (heartbeat, `archive_timeout=60`, 5 GB volume, daily differential) cannot restore a point about 5 hours or more after its base backup. Ask whether fork volumes can be sized at restore time.

After any change, re-run this drill with a target at least 8 hours after the latest base backup.

## Re-run results (2026-11-18)

Run after the 2-hourly differential schedule had been live for about 24 hours ([BACKUP_SCHEDULE_SPEC.md](BACKUP_SCHEDULE_SPEC.md)). Two targets, as that spec requires. Times are UTC.

**Outcome:** target A (worst case inside the 2-hour cadence) **restored** and matched. Target B (a point more than a day back) **failed** with the same `No space left on device` as 2026-11-17. The restore is therefore proven for recent points but **not yet for the whole window**.

| Step | Result |
| --- | --- |
| Preflight | Stanza `ok`. Since 2026-11-17 20:39: 12 differentials (20:39, 22:39, then every 2 h to 08:42; 12:09 to 20:09 on the new chain) plus the scheduled weekly full `20261118-100849F` at 10:08. Nothing expired early; every backup back to the 2026-11-04 full is still listed. Archiver `failed_count` 0. Service IDs: `Postgres` `b5cdda94-…`, `web` `160804a7-…`. |
| Source fingerprint | Identical for both targets: latest migration `20261117020000` (23 migrations); books 30,042; book_copies 30,044; authors 15,723; genres 25; users created by T: 2; reviews created by T: 1; index `account_email_rate_limits_expires_at_idx` present; 37 public tables; roles `chantels_storefront` and `postgres`. No rows changed after either target (the newest user is from 2026-09-25, the newest review from 2026-10-17), so no difference needed explaining. |

### Target A: `2026-11-18T22:04:54Z` (1 h 55 m after differential `20261118-100849F_20261118-200954D`)

- Restore issued 22:10:55 as `postgres-restore-drill-20261118-a`. pgBackRest restored backup set `…_20261118-200954D` (108.5 MB); replay started at redo LSN `50/09`, the backup's own start.
- Replayed 122 segments (`50/09` → `50/82`, about 1.9 GB) in 54 s. `recovery stopping before commit of transaction 1677, time 2026-11-18 22:10:08` (the next commit after T, written by the 22:10 differential); last completed transaction 21:33:25. New timeline 2; `ready to accept connections` at 22:13:18.
- **Recovery time:** 2 m 23 s from issue to ready; first successful query (`pg_is_in_recovery()` = `f`) observed at 22:13:35, 2 m 40 s after issue.
- Fork checks: `archive_mode` `off`, `archive_command` `(disabled)`; every fingerprint above matched exactly, including the index, 37 tables and both roles.

### Target B: `2026-11-17T20:50:00Z` (11 min after differential `20261111-100716F_20261117-203904D`)

- Restore issued 21:02:15 as `postgres-restore-drill-20261118-b`. pgBackRest restored backup set `…_20261117-203904D`, but Postgres reported `starting backup recovery with redo LSN 4A/83000028`: about 120 segments **before** that backup's own WAL start (`4A/FB`). That backup ran one minute after `Postgres` restarted for the schedule change (20:38:02); why its redo point lags is not established.
- First boot replayed 280 segments (`4A/83` → `4B/9A`, about 4.4 GB) with no restartpoint logged, then `archive-get` failed with `[28] No space left on device` writing `pg_wal/RECOVERYXLOG`. The volume was 4,975 of 5,000 MB used.
- The container then crash-looped: each boot reached `recovery stopping before commit of transaction 1424, time 2026-11-17 22:39:31` (on an idle database the next commit after T was the 22:39 differential) and died at promotion with `FATAL: could not write to file "pg_wal/xlogtemp.84": No space left on device`. It never accepted a query, so there is no recovery time and no fork fingerprint.
- Railway's restore image has changed since 2026-11-17. It now logs `volume 4614 MiB; … queue-max=2307 MiB` (the async `archive-get` spool may take half the volume, up from 1 GiB) and asks for `checkpoint_timeout=30s max_wal_size=512MB` restartpoints, "best effort". No restartpoint was logged in either fork.

### Production and teardown

- Production unaffected: `/readyz` `ready`; web `DATABASE_URL` host still `postgres.railway.internal`; archiver `failed_count` 0 (last archived 22:14:07); stanza `ok`.
- Fork B service `4c0c3f8a-…` deleted at 21:14 after the ID and name check; its volume `postgres-restored-Y6Dq` (`00720d6a-…`) deleted (pending deletion).
- Fork A service `2a10dfc4-…` deleted at 22:15 after the same check; its volume `postgres-restored-X2LV` (`f4e75ac2-…`) deleted (pending deletion).
- The `Postgres` volume `postgres-volume` (`881d24e0-…`) was untouched. Yesterday's `postgres-restored` (`bda488ee-…`) is still pending deletion.

### What this shows

- A restore whose base backup's redo point is its own start replays at most about 2 hours of WAL and fits: target A used about 1.9 GB of WAL plus the 108 MB data directory.
- On an idle database a time target stops only at the next commit, which is the next differential, so every target replays up to the following backup. The 2-hour cadence bounds that, **provided** the base backup's redo point is not far behind its start.
- Target B's base backup had a redo point about 2 hours early, so it needed about 4 hours of WAL, and the larger spool left even less room. Points restored from that backup, and any other backup with the same lag, are not reachable today.

### Next

1. Check, read-only, whether other differentials carry a lagging redo point (`backup_label` in the repository, or a restore's first log line), or only the one taken right after the restart.
2. Re-run target B (or a target inside a later retained differential, after 2026-11-17 22:39) once the cause is understood. This needs a new go-ahead.
3. Report the spool sizing and missing restartpoints to Railway (option 4 above).

