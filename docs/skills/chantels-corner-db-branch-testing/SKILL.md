---
name: chantels-corner-db-branch-testing
description: Safely test Chantel's Corner Rust/Postgres changes against a disposable Neon branch, especially authentication, password, migration, and other DB-backed flows.
---

# Chantel's Corner database-branch testing

Use this local skill for DB-backed verification that would otherwise touch the live Neon database. It is especially relevant to password signup/reset changes, migrations, sessions, email outbox behavior, and account lifecycle tests.

## Safety boundary

- Never point schema-dropping integration tests at the Railway production `DATABASE_URL`.
- Do not create a new Neon project for ordinary test isolation. Create a temporary branch in the existing `bookstore` project, parented from the current production branch.
- Keep `NEON_API_KEY` and connection URIs in process or ignored local files only. Never print, commit, or paste them into chat.
- Delete the temporary branch after verification unless the user explicitly asks to keep it.
- Preserve unrelated dirty worktree changes. Do not reset, stash destructively, or overwrite them.

## Workflow

1. Inspect `git status`, the current branch, relevant specs, and the existing `setup/.secrets.demo.env`. Do not source that file blindly: unquoted values may contain `&`. Extract individual values safely, or use a parser that honors its quoting.
2. Read `NEON_API_KEY` from the ignored secrets file without printing it. Query `https://console.neon.tech/api/v2/projects` and identify the existing `bookstore` project. Query its branches and identify the production parent.
3. Create a uniquely named temporary branch, for example `codex-auth-test-YYYYMMDD`, from production with a read/write endpoint. Record only the branch id and non-secret name in local process state.
4. Obtain a non-pooled branch connection URI from Neon’s `connection_uri` endpoint. Pass it as an environment variable only; never write it to tracked files or output it.
5. Run the narrowest meaningful checks first:
   - `cargo fmt --all -- --check`
   - `cargo check --workspace --locked`
   - focused unit tests for the changed validator
   - focused DB-backed lifecycle tests with `cargo test <filter> --workspace --locked -- --test-threads=1`
6. Run the broader PostgreSQL-backed suite on the temporary branch when the focused checks pass. Treat failures caused by stale fixtures as code/test maintenance findings; update fixtures to satisfy the current policy and rerun.
7. Verify the public deployment only after tests pass: inspect the Railway deployment status and request the public `/readyz`. Do not claim “live” from a successful local build.
8. Delete the temporary Neon branch through the Neon API, then verify deletion. If cleanup fails, report the branch name/id clearly without exposing credentials.

## Password-policy checks

Signup and password reset must call the same Rust validator in `src/auth.rs`. Keep browser `minlength`/`maxlength` hints aligned with its constants, but do not treat browser validation as security validation. Test minimum length, maximum length, Unicode character counting, the 3-of-4 category rule, common-password rejection, matching confirmation, and reset-token non-consumption on validation failure.

For the current Amazon-style policy, valid passwords are 6–128 Unicode characters and contain at least 3 of uppercase letters, lowercase letters, digits, or non-alphanumeric symbols. Existing password fixtures must include at least 3 categories.

## Local evidence

Record commands, pass/fail results, branch lifecycle, and deployment health. Redact all secrets and full connection strings. Distinguish verified, blocked, and not-run checks.
