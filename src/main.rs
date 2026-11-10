mod db;

use axum::{
    extract::State,
    Form, Router,
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
};
use tower_http::services::ServeDir;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use rand::seq::SliceRandom;
use serde::Deserialize;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;

const TRIVIA_API_URL: &str = "https://opentdb.com/api.php?amount=1&encode=url3986";
const PLAYER_COOKIE: &str = "player_id";

#[derive(Clone)]
struct AppState {
    db: SqlitePool,
}

#[tokio::main]
async fn main() {
    let options = SqliteConnectOptions::from_str("sqlite:trivia.db")
        .expect("valid sqlite URL")
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .expect("connect to sqlite");
    sqlx::migrate!().run(&pool).await.expect("run migrations");

    let app = Router::new()
        .nest_service("/static", ServeDir::new("static"))
        .route("/", get(home))
        .route("/play", get(play))
        .route("/join", post(join))
        .route("/answer", post(check_answer))
        .with_state(AppState { db: pool });

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Listening on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}

#[derive(Debug, Deserialize)]
struct TriviaApiResponse {
    response_code: i32,
    results: Vec<TriviaQuestion>,
}

#[derive(Debug, Deserialize)]
struct TriviaQuestion {
    category: String,
    #[serde(rename = "type")]
    question_type: String,
    difficulty: String,
    question: String,
    correct_answer: String,
    incorrect_answers: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct JoinForm {
    name: String,
}

#[derive(Debug, Deserialize)]
struct AnswerForm {
    answer: String,
    correct_answer: String,
    question: String,
}

fn player_id_from_jar(jar: &CookieJar) -> Option<i64> {
    jar.get(PLAYER_COOKIE)?.value().parse().ok()
}

fn player_cookie(player_id: i64) -> Cookie<'static> {
    Cookie::build((PLAYER_COOKIE, player_id.to_string()))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .build()
}

fn decode_opentdb(s: &str) -> String {
    urlencoding::decode(s)
        .map(|cow| cow.into_owned())
        .unwrap_or_else(|_| s.to_string())
}

async fn fetch_question() -> Result<TriviaQuestion, String> {
    let response = reqwest::get(TRIVIA_API_URL)
        .await
        .map_err(|e| format!("Could not reach trivia API: {e}"))?;

    let body: TriviaApiResponse = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from trivia API: {e}"))?;

    if body.response_code != 0 {
        return Err(format!(
            "Trivia API returned error code {}",
            body.response_code
        ));
    }

    body.results
        .into_iter()
        .next()
        .ok_or_else(|| "Trivia API returned no questions".to_string())
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn site_header(active: Option<&str>, player_name: Option<&str>) -> String {
    let home_aria = if active == Some("home") {
        r#" aria-current="page""#
    } else {
        ""
    };
    let play_aria = if active == Some("play") {
        r#" aria-current="page""#
    } else {
        ""
    };
    let player_list = player_name
        .map(|name| {
            format!(
                r#"<ul><li><small>Signed in as <strong>{}</strong></small></li></ul>"#,
                escape_html(name)
            )
        })
        .unwrap_or_default();
    format!(
        r#"<header class="container">
  <nav>
    <ul>
      <li><strong><a href="/">Trivia</a></strong></li>
    </ul>
    <ul>
      <li><a href="/"{home_aria}>Home</a></li>
      <li><a href="/play"{play_aria}>Play</a></li>
    </ul>
    {player_list}
  </nav>
</header>"#
    )
}

fn layout(title: &str, body: &str) -> Html<String> {
    layout_with_nav(title, body, None)
}

fn layout_with_nav(title: &str, body: &str, nav: Option<(&str, Option<&str>)>) -> Html<String> {
    let (active, player_name) = nav
        .map(|(page, name)| (Some(page), name))
        .unwrap_or((None, None));
    let header = site_header(active, player_name);
    Html(format!(
        r#"<!DOCTYPE html>
<html lang="en" data-theme="light">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title} · Trivia</title>
  <link rel="stylesheet" href="/static/pico.min.css">
  <link rel="stylesheet" href="/static/app.css">
</head>
<body>
{header}
<main class="container">
{body}
</main>
</body>
</html>"#
    ))
}

async fn logged_in_player(
    state: &AppState,
    jar: &CookieJar,
) -> Result<db::Player, Html<String>> {
    let player_id = player_id_from_jar(jar).ok_or_else(|| name_page(None))?;
    db::get_player(&state.db, player_id)
        .await
        .map_err(|e| {
            layout(
                "Error",
                &format!(
                    r#"<h1>Error</h1><p>{}</p><p><a href="/">Back</a></p>"#,
                    escape_html(&e.to_string())
                ),
            )
        })?
        .ok_or_else(|| name_page(Some("Please enter your name to play.")))
}

async fn player_name_from_jar(state: &AppState, jar: &CookieJar) -> Option<String> {
    let id = player_id_from_jar(jar)?;
    let player = db::get_player(&state.db, id).await.ok()??;
    Some(player.name)
}

fn name_page(note: Option<&str>) -> Html<String> {
    let note_html = note
        .map(|n| format!("<p>{}</p>", escape_html(n)))
        .unwrap_or_default();
    layout(
        "Welcome",
        &format!(
            r#"<hgroup>
  <h1>Welcome</h1>
  <p>Enter your name to track your score.</p>
</hgroup>
{note_html}
<form method="post" action="/join">
  <label>
    Your name
    <input name="name" type="text" required minlength="1" maxlength="64" autofocus placeholder="Ada Lovelace">
  </label>
  <button type="submit">Start playing</button>
</form>"#
        ),
    )
}

async fn join(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<JoinForm>,
) -> impl IntoResponse {
    let name = form.name.trim();
    if name.is_empty() {
        return (jar, name_page(Some("Please enter a name."))).into_response();
    }

    let player_id = match db::create_player(&state.db, name).await {
        Ok(id) => id,
        Err(e) => {
            return (
                jar,
                layout(
                    "Error",
                    &format!(
                        r#"<h1>Error</h1><p>Could not save your name: {}</p><p><a href="/">Back</a></p>"#,
                        escape_html(&e.to_string())
                    ),
                ),
            )
                .into_response();
        }
    };

    let jar = jar.add(player_cookie(player_id));
    (jar, Redirect::to("/")).into_response()
}

async fn home(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    let player_name = player_name_from_jar(&state, &jar).await;

    let attempts = db::all_attempts(&state.db).await.unwrap_or_default();

    let table_body = if attempts.is_empty() {
        r#"<tr><td colspan="6">No attempts yet. Be the first to play!</td></tr>"#.to_string()
    } else {
        attempts
            .iter()
            .map(|a| {
                let result = if a.correct != 0 {
                    r#"<span class="result-yes">Correct</span>"#
                } else {
                    r#"<span class="result-no">Wrong</span>"#
                };
                format!(
                    "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{result}</td></tr>",
                    escape_html(&a.answered_at),
                    escape_html(&a.player_name),
                    escape_html(&a.question),
                    escape_html(&a.player_answer),
                    escape_html(&a.correct_answer),
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    layout_with_nav(
        "Home",
        &format!(
            r#"<hgroup>
  <h1>All attempts</h1>
  <p>Every question answered across all players.</p>
</hgroup>
<p><a href="/play" role="button">Continue playing</a></p>
<div class="table-scroll">
  <table>
    <thead>
      <tr><th>When</th><th>Player</th><th>Question</th><th>Answered</th><th>Correct</th><th>Result</th></tr>
    </thead>
    <tbody>
      {table_body}
    </tbody>
  </table>
</div>"#,
        ),
        Some(("home", player_name.as_deref())),
    )
    .into_response()
}

async fn play(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    let player = match logged_in_player(&state, &jar).await {
        Ok(player) => player,
        Err(page) => return page.into_response(),
    };

    match fetch_question().await {
        Ok(q) => question_page(&player.name, &q).into_response(),
        Err(e) => layout_with_nav(
            "Play",
            &format!(r#"<h1>Play</h1><p>{e}</p><p><a href="/play">Try again</a></p>"#, e = escape_html(&e)),
            Some(("play", Some(player.name.as_str()))),
        )
        .into_response(),
    }
}

fn question_page(player_name: &str, q: &TriviaQuestion) -> Html<String> {
    let category = escape_html(&decode_opentdb(&q.category));
    let question = escape_html(&decode_opentdb(&q.question));
    let question_raw = escape_html(&decode_opentdb(&q.question));
    let difficulty = escape_html(&q.difficulty);

    let mut choices = q.incorrect_answers.clone();
    choices.push(q.correct_answer.clone());
    choices.shuffle(&mut rand::rng());

    let mut options = String::new();
    for choice in &choices {
        let decoded = decode_opentdb(choice);
        let value = escape_html(&decoded);
        options.push_str(&format!(
            r#"<label><input type="radio" name="answer" value="{value}" required> {value}</label>"#,
        ));
    }

    let correct = escape_html(&decode_opentdb(&q.correct_answer));
    let input_type = if q.question_type == "boolean" {
        "true/false"
    } else {
        "multiple choice"
    };

    layout_with_nav(
        "Play",
        &format!(
            r#"<hgroup>
  <h1>Play</h1>
  <p>{category} · {difficulty} · {input_type}</p>
</hgroup>
<article>
  <header><strong>{question}</strong></header>
  <form method="post" action="/answer">
    <input type="hidden" name="correct_answer" value="{correct}">
    <input type="hidden" name="question" value="{question_raw}">
    <fieldset>
      <legend>Choose an answer</legend>
      {options}
    </fieldset>
    <button type="submit">Submit answer</button>
  </form>
</article>
<p><a href="/play">Skip to another question</a></p>"#,
        ),
        Some(("play", Some(player_name))),
    )
}

async fn check_answer(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<AnswerForm>,
) -> impl IntoResponse {
    let Some(player_id) = player_id_from_jar(&jar) else {
        return name_page(Some("Please enter your name before answering.")).into_response();
    };

    let Ok(Some(player)) = db::get_player(&state.db, player_id).await else {
        return name_page(Some("Session expired; enter your name again.")).into_response();
    };

    let correct = form
        .correct_answer
        .trim()
        .eq_ignore_ascii_case(form.answer.trim());

    if let Err(e) = db::record_attempt(
        &state.db,
        player_id,
        &form.question,
        &form.answer,
        &form.correct_answer,
        correct,
    )
    .await
    {
        return layout(
            "Error",
            &format!(
                r#"<h1>Error</h1><p>Could not save your result: {}</p><p><a href="/">Back</a></p>"#,
                escape_html(&e.to_string())
            ),
        )
        .into_response();
    }

    let stats = db::player_stats(&state.db, player_id)
        .await
        .unwrap_or(db::PlayerStats {
            total: 0,
            correct: 0,
        });

    let message = if correct {
        "Correct! Nice work."
    } else {
        "Not quite."
    };

    let body = format!(
        r#"<h1>{message}</h1>
<dl>
  <dt>Your answer</dt>
  <dd>{yours}</dd>
  <dt>Correct answer</dt>
  <dd>{correct}</dd>
  <dt>Your record</dt>
  <dd><strong>{stats_correct}</strong> correct out of <strong>{stats_total}</strong> answered.</dd>
</dl>
<p>
  <a href="/play" role="button">Play another question</a>
  <a href="/" role="button" class="secondary">Back to home</a>
</p>"#,
        message = message,
        yours = escape_html(&form.answer),
        correct = escape_html(&form.correct_answer),
        stats_correct = stats.correct,
        stats_total = stats.total,
    );

    layout_with_nav("Result", &body, Some(("play", Some(player.name.as_str())))).into_response()
}
