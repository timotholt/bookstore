# Backup Schedule Spec

Status: proposed 2026-11-17. Not applied.

## Problem

The 2026-11-17 restore drill ([RESTORE_DRILL_SPEC.md](RESTORE_DRILL_SPEC.md)) failed. A restored service starts from the most recent base backup before the target, then replays WAL forward from there. Railway's Postgres template makes one 16 MB WAL segment per minute even when the database is idle (60-second `archive_timeout` plus an idle heartbeat). The restore writes those segments onto the fork's 5 GB volume, and the volume filled after about 4.4 hours of replay.

Today's schedule leaves most targets beyond that limit:

| Setting (env var on `Postgres`) | Default, in use | Effect |
| --- | --- | --- |
| `WAL_BACKUP_FULL_INTERVAL_HOURS` | 168 | Weekly full backup |
| `WAL_BACKUP_DIFF_INTERVAL_HOURS` | 24 | Daily differential, around 10:07 UTC |
| `WAL_BACKUP_RETENTION_FULL` | 4 | Keeps 4 fulls. WAL is kept for the same span, so the PITR window is about 4 weeks. |
| `WAL_BACKUP_RETENTION_DIFF` | 14 | Keeps 14 differentials (2 weeks at daily) |

Two gaps:

1. **Within a day:** only targets up to about 4 hours after the daily differential restore. Roughly 20 hours a day cannot.
2. **Older than 2 weeks:** differentials for that period have expired, so a restore replays from a weekly full, which can be days of WAL. That cannot fit either.

`railway postgres pitr schedule set` does not help. It schedules Railway volume snapshots (daily, weekly or monthly), not the pgBackRest base backups that PITR restores from. These four variables are the pgBackRest controls; the template's backup watcher and `wrapper.sh` read them at container start.

## Goal

Any point in the PITR window can be restored. The worst case needs at most about 2 hours of WAL replay, well within a 5 GB fork volume.

## Design

Set on the `Postgres` service:

```text
WAL_BACKUP_DIFF_INTERVAL_HOURS=2
WAL_BACKUP_RETENTION_DIFF=336
```

Leave fulls weekly with 4 kept, so the window stays about 4 weeks.

- **Why 2 hours:** the fork filled after about 4.4 hours of replay (about 4.5 GB, which includes pgBackRest's 1 GiB `archive-get` spool). Two hours is at most about 2 GB of WAL plus the spool and the roughly 110 MB data directory, around 3.2 GB on a 5 GB volume. Three hours would leave about 1 GB of headroom; 2 hours leaves about 1.8 GB.
- **Why 336:** 4 weeks × 7 days × 12 differentials a day. This keeps a differential at most 2 hours before every point in the window. With fewer, older points fall back to replaying from a full.

### Cost and load (estimate, check after a week)

- A differential backup takes about 15 s and uploads about 18–21 MB compressed today. That grows as data changes during the week and resets at each full.
- **Bucket storage:** up to 336 differentials of about 20 MB each, roughly 7 GB at steady state, against about 0.3 GB of backups today. The WAL archive itself is small, because idle segments compress to a few KB.
- **Egress** from the database service for uploads: about 12 × 20 MB = about 240 MB a day.
- **Load:** one extra checkpoint (`start-fast=y`) and a short read of the data directory every 2 hours. That is negligible at this size.
- At Railway's bucket and egress rates this should be well under $1 a month. Check the real cost on the Railway usage page after the first week.

## Rollout

The variables are read at container start, so changing them **restarts the `Postgres` service**. The volume moves to the new container, so expect about 1–2 minutes when `/readyz` fails and pages error. The web service is not redeployed. Its pool reconnects by itself, and the current deployment keeps running.

1. Read-only preflight: `pgbackrest info` stanza `ok`; archiver `failed_count` 0; note the latest backup.
2. Set both variables with one command, `railway variable set ... --service Postgres`, at a quiet time. Railway redeploys `Postgres` once.
3. Wait for the `Postgres` deploy to become healthy, then for `/readyz` to say `ready`.
4. Verify:
   - `/etc/pgbackrest/pgbackrest.conf` shows `repo1-retention-diff=336`.
   - The watcher startup log shows the 2-hour differential cadence.
   - A new differential appears in `pgbackrest info` within 2 hours. After 24 hours there are about 12 new differentials, and none have been expired early.
   - The archiver still shows 0 failures.
5. Update `INFRASTRUCTURE_SPEC.md` (backups row and open item) and mark this spec applied.

**Rollback:** delete both variables (one more restart). The defaults return, and existing backups are kept.

## Proving it: re-run the restore drill

This is a separate go-ahead, because each restore creates a billed fork for a few minutes. Run [RESTORE_DRILL_SPEC.md](RESTORE_DRILL_SPEC.md) at least 24 hours after rollout, with two targets:

1. **Worst case inside the cadence:** a target about 1 h 55 m after the most recent differential, with the migration-index or row-count check.
2. **An older point:** a target more than a day back, after rollout, so it depends on a retained differential and not on the latest one.

Both must reach their target and match the fingerprints. Record the recovery times.

## Not chosen now

- **Larger volume:** fixes it too, but the storage is billed all the time to cover restores only.
- **`WAL_HEARTBEAT_DISABLED=1`:** cuts WAL on an idle database, but the worst-case data loss rises to the checkpoint interval (about 5 min).
- **Reporting it to Railway:** still worth doing. The default template cannot restore most of its own window on its default volume. That is separate from this change.
