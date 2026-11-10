# axum-test

A small trivia web app built with [Axum](https://github.com/tokio-rs/axum). It fetches random questions from the [Open Trivia Database](https://opentdb.com/), lets players submit answers, and stores results in SQLite.

## Prerequisites

- [Rust](https://rustup.rs/) (2021 edition or newer; this project uses edition 2024)
- Network access (trivia API and first-time `cargo` dependency download)

Optional, for auto-restart while editing:

```bash
cargo install cargo-watch
```

## Run the app

From this directory:

```bash
cargo run
```

The server listens on **http://localhost:3000**.

Open that URL in a browser. Enter your name to play; **Home** shows all players’ attempts, **Play** loads a new question.

## Development (reload on save)

If `cargo-watch` is installed:

```bash
cargo dev
```

This alias (in `.cargo/config.toml`) watches `src/`, `migrations/`, and `static/`, then re-runs the server when files change. Refresh the browser after each restart.

## Data & static files

- **SQLite:** `trivia.db` is created in the project root on first run. Migrations in `migrations/` apply automatically at startup.
- **CSS:** [Pico CSS](https://picocss.com/) and `static/app.css` are served from `/static/`.

To reset the database, stop the server, delete `trivia.db`, and start again.

## Project layout

| Path | Purpose |
|------|---------|
| `src/main.rs` | Routes, HTML, trivia API client |
| `src/db.rs` | SQLite queries |
| `migrations/` | Schema changes |
| `static/` | Pico CSS and app styles |

## Notes

- The Open Trivia DB API rate-limits to about one request per five seconds per IP; rapid “new question” clicks may fail briefly.
- Trivia questions are loaded from `https://opentdb.com/api.php?amount=1&encode=url3986`.
