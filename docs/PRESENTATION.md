# Repository presentation

[Project showcase](../README.md) · [Documentation index](README.md)

The README is organized for a quick product tour followed by a technical review: cover, screenshots and walkthrough, architecture, engineering decisions, current scope, and local setup.

## Asset inventory

| Asset | Purpose | Source |
| --- | --- | --- |
| [Repository cover](assets/repository-cover.png) | README title artwork | AI-generated bookstore illustration |
| [Social preview](assets/social-preview.jpg) | GitHub link-sharing preview; 1280 × 640, under 1 MB | Resized and encoded copy of the cover |
| [Homepage](assets/homepage.png) | Storefront overview | Actual local application screenshot |
| [Catalog](assets/catalog.png) | Science Fiction filter results | Actual local application screenshot |
| [Book detail](assets/book-detail.png) | Dune copy information | Actual local application screenshot |
| [Cart](assets/cart.png) | A sample book added to the cart | Actual local application screenshot |
| [Account](assets/account.png) | Account navigation for a fictional reader | Actual local application screenshot |
| [Walkthrough GIF](assets/shopping-walkthrough.gif) | Inline README demonstration | Browser recording of the local application |
| [Walkthrough video](assets/shopping-walkthrough.mp4) | Higher-quality downloadable demonstration | Same browser recording |
| [Architecture poster](study-assets/bookstore-stack-professional.png) | Explain the stack | AI-generated architecture illustration |

The screenshots use the seeded demo catalog at a 1440 × 1000 viewport. The account screenshot uses a fictional Avery Reader account created only in an isolated local PostgreSQL database. No real customer information is shown. Storefront ratings, promotions, and other sample copy should not be interpreted as evidence of real commerce.

The architecture poster simplifies HTTP response plumbing. SQLx results return to the Rust handler before the handler prepares view data for Askama; Axum delivers the resulting response to the browser.

## Refresh the product captures

1. Run the current Rust application against a disposable local PostgreSQL database. Follow the [development guide](DEVELOPMENT.md). A separate database prevents demo carts and accounts from mixing with other data.
2. Install `agent-browser` and `ffmpeg` if they are not already available. These are optional presentation tools, not application dependencies.
3. Run the capture script against that local server:

   ```bash
   bash docs/capture-showcase.sh http://127.0.0.1:8083
   ```

   If the CLI is not on `PATH`, set `AGENT_BROWSER_BIN` to its executable path. The script uses a fresh browser session, captures the homepage, filters to Science Fiction, opens the seeded Dune record, adds it to the cart, and exports the four screenshots plus the GIF and MP4. It rejects non-local URLs and closes its browser session afterward.

4. For the account screenshot, create a fictional local account and capture `/account` at the same viewport. Do not publish passwords or browser storage state.
5. Inspect each capture. Check loaded covers, legible text, no tool overlays, and correct cart contents. Keep screenshots of actual behavior; use illustrations only for covers and explanatory diagrams.
6. Update any affected captions, scope notes, and verification details. Confirm Markdown links still point to real files.

## GitHub About panel

Suggested description:

> A Rust bookstore portfolio: Axum + Askama + HTMX, PostgreSQL-backed carts and accounts, and a server-rendered shopping experience.

Suggested topics: `rust`, `axum`, `askama`, `htmx`, `postgresql`, `sqlx`, `bookstore`, `portfolio`, `server-side-rendering`.

The About homepage links to the [verified public demo](https://chantelscorner.com/). Public homepage and database readiness were independently checked on 2026-09-19. Screenshots and the walkthrough remain captures of the isolated local demo.

## Social preview

The cover was uploaded and visually verified in GitHub repository settings on 2026-09-19. The project description and stack topics were also applied.

Upload [social-preview.jpg](assets/social-preview.jpg) under repository **Settings → General → Social preview**. Merely committing an image does not configure the GitHub social preview.

The exported image follows GitHub's [recommended 1280 × 640 size and under-1-MB limit](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/customizing-your-repositorys-social-media-preview). The README embeds the larger cover separately.

## Illustration provenance

The cover and architecture images were created using the built-in image-generation tool. Their prompts are preserved with the assets:

- [Cover prompt](assets/repository-cover-prompt.txt)
- [Professional architecture prompt](study-assets/image-prompt-professional.txt)

## Verification for this presentation

- Captures taken from the current Rust application on 2026-09-19 using an isolated local PostgreSQL database.
- `cargo check --locked`: passed.
- `cargo test --locked -- --test-threads=1`: 34 passed, 0 failed.
- GitHub Markdown rendering checked in a browser: all 12 README images loaded.
- Local documentation links and capture-script shell syntax checked.
- The browser walkthrough exercises catalog filtering, a book-detail page, adding to cart, and cart rendering.

These checks support the local demo. The CI badge reports the status of GitHub's `main` branch separately.
