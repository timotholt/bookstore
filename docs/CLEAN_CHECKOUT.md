# Verify a clean checkout

This procedure verifies that the committed repository contains everything needed to build and run Chantel’s Corner. It uses a fresh clone, a disposable PostgreSQL database, and the production Dockerfile. No existing `.env`, local assets, database, or build output is copied into the clone.

## Prerequisites

- Git.
- Stable Rust with Cargo, rustfmt, and Clippy (`rustup component add rustfmt clippy`).
- Docker Engine or Docker Desktop, running.
- A Bash shell and curl. On Windows, use WSL2.
- Internet access to GitHub, Cargo registries, and container/package registries.
- Available local ports 55432 and 8081.

Run the following blocks in the same Bash session. Container and network names must not already exist; if they do, finish or clean up the earlier verification first.

## Clone only committed files

```bash
bash
set -euo pipefail
cd "$(mktemp -d)"
git clone https://github.com/timotholt/bookstore.git
cd bookstore
git rev-parse HEAD
export CARGO_TARGET_DIR="$PWD/target"
```

This tests the default branch pushed to GitHub. To test a different published branch or commit, run `git checkout <ref>` before continuing and record `git rev-parse HEAD` again. Uncommitted and unpushed work is intentionally absent. Cargo may reuse downloaded dependencies, but compiled artifacts are isolated to this clone.

## Start a disposable database

These credentials are throwaway local test values. Do not substitute a production database: startup applies migrations and seeds the catalog, and tests create and drop isolated schemas.

```bash
docker network create chantels-clean-net
docker run --detach --rm --name chantels-clean-db \
  --network chantels-clean-net \
  -e POSTGRES_PASSWORD=localtest \
  -e POSTGRES_DB=chantels_test \
  -p 127.0.0.1:55432:5432 postgres:16

for attempt in {1..60}; do
  if docker exec chantels-clean-db pg_isready -U postgres -d chantels_test; then
    break
  fi
  sleep 1
done
docker exec chantels-clean-db pg_isready -U postgres -d chantels_test
export DATABASE_URL='postgres://postgres:localtest@127.0.0.1:55432/chantels_test?sslmode=disable'
```

## Build and test the entire workspace

```bash
cargo fmt --all -- --check
cargo check --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --release --locked
cargo test --workspace --locked -- --test-threads=1
docker build --no-cache --tag chantels-corner:clean .
```

All commands must succeed. `--workspace` includes the application and setup tooling; `--locked` preserves the committed dependency resolution. The Docker build separately packages the application binary and runtime static files. There is no separate Node or frontend build step: Askama templates compile with Rust, and JavaScript/CSS/assets are included in the image.

## Run the packaged product

```bash
docker run --detach --rm --name chantels-clean-app \
  --network chantels-clean-net \
  -p 127.0.0.1:8081:8080 \
  -e ADDR=0.0.0.0:8080 \
  -e DATABASE_URL='postgres://postgres:localtest@chantels-clean-db:5432/chantels_test?sslmode=disable' \
  chantels-corner:clean

for attempt in {1..60}; do
  if curl --fail --silent http://127.0.0.1:8081/readyz >/dev/null; then
    break
  fi
  sleep 1
done
curl --fail --show-error http://127.0.0.1:8081/readyz
for route in / /healthz /search /cart /signup /login /styles.css /app.js /assets/htmx.min.js; do
  curl --fail --silent --show-error "http://127.0.0.1:8081$route" >/dev/null
done
curl --fail --silent --show-error -H 'HX-Request: true' \
  http://127.0.0.1:8081/catalog >/dev/null
```

Open <http://127.0.0.1:8081>. Confirm that styling and images load, search/filter the catalog, follow a book link, add an available copy to the cart, change its quantity, and try signup/login with a fictional test account. Checkout is a preview; the current product does not place orders or collect payments.

Passing builds proves compilation and packaging. Passing tests and these runtime checks additionally exercises migrations, database connectivity, routes, static files, and basic user flows. Record the commit SHA and any failed step when reporting results. For startup failures, inspect `docker logs chantels-clean-app` and `docker logs chantels-clean-db`.

## Cleanup

Run this even if a previous step failed, from a new terminal if necessary:

```bash
docker stop chantels-clean-app
docker stop chantels-clean-db
docker network rm chantels-clean-net
docker image rm chantels-corner:clean
```

An already-stopped or never-created container may report that it does not exist. No persistent database volume was created. The temporary clone remains available for inspecting results.

## Build from GitHub with one button

1. Open [Actions → CI](https://github.com/timotholt/bookstore/actions/workflows/ci.yml).
2. Click **Run workflow**, choose a branch, and click the green **Run workflow** button.
3. Open the run to see each build/test step and its result.
4. Download artifacts from the completed run's **Artifacts** section:
   - `chantels-corner-linux-amd64-<commit>`: the verified Docker image, SHA-256 checksum, and exact checked-out commit.
   - `ci-evidence-<commit>`: build, test, smoke-check, application, and database logs, including available logs from failed runs.

Artifacts are retained for seven days. The image artifact is only uploaded after all build, test, and packaged-app smoke checks succeed. Download and extract the artifact ZIP, then load its image:

```bash
gunzip -c chantels-corner-linux-amd64.tar.gz | docker load
```

The image is Linux/AMD64. Other CPU architectures require Docker emulation. Run it against a disposable PostgreSQL database using the packaged-product instructions above, substituting `chantels-corner:ci` for `chantels-corner:clean`.

The manual button becomes available once this workflow is committed to the repository's default branch. Running it requires repository write access. See [GitHub's manual-run documentation](https://docs.github.com/en/actions/managing-workflow-runs-and-deployments/managing-workflow-runs/manually-running-a-workflow).

## Automated coverage

[GitHub Actions](../.github/workflows/ci.yml) checks fresh checkouts on manual runs, pull requests, and pushes to `main`: formatting, compilation, linting, workspace build, tests against PostgreSQL, Docker packaging, and HTTP smoke checks of the running image. The smoke script checks database readiness, pages, static assets, an HTMX catalog response, and a book linked from the seeded homepage. It does not replace the manual browser interaction checks above.

The workflow builds and uploads run artifacts; it does not deploy the application or publish a release. A clean build is not a claim that planned features are implemented; see the [current product scope](../README.md).
