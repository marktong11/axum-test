use axum::{
    Form, Router,
    response::{Html, IntoResponse},
    routing::{get, post},
};
use rand::seq::SliceRandom;
use serde::Deserialize;

const TRIVIA_API_URL: &str = "https://opentdb.com/api.php?amount=1&encode=url3986";

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(home))
        .route("/answer", post(check_answer));

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

fn layout(title: &str, body: &str) -> Html<String> {
    Html(format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title}</title>
  <style>
    :root {{ font-family: system-ui, sans-serif; line-height: 1.5; color: #1a1a1a; background: #f6f7f9; }}
    body {{ max-width: 36rem; margin: 2rem auto; padding: 0 1rem; }}
  </style>
</head>
<body>
{body}
</body>
</html>"#
    ))
}

async fn home() -> impl IntoResponse {
    match fetch_question().await {
        Ok(q) => question_page(&q),
        Err(e) => layout(
            "Trivia",
            &format!(
                r#"<h1>Trivia</h1><p>{e}</p><p><a href="/">Try again</a></p>"#,
                e = escape_html(&e)
            ),
        ),
    }
}

fn question_page(q: &TriviaQuestion) -> Html<String> {
    let category = escape_html(&decode_opentdb(&q.category));
    let question = escape_html(&decode_opentdb(&q.question));
    let difficulty = escape_html(&q.difficulty);

    let mut choices = q.incorrect_answers.clone();
    choices.push(q.correct_answer.clone());
    choices.shuffle(&mut rand::rng());

    let mut options = String::new();
    for choice in &choices {
        let decoded = decode_opentdb(choice);
        let value = escape_html(&decoded);
        options.push_str(&format!(
            r#"<label style="display:block;margin:.5rem 0;"><input type="radio" name="answer" value="{value}" required> {value}</label>"#,
        ));
    }

    let correct = escape_html(&decode_opentdb(&q.correct_answer));
    let input_type = if q.question_type == "boolean" {
        "true/false"
    } else {
        "multiple choice"
    };

    layout(
        "Trivia",
        &format!(
            r#"<h1>Trivia</h1>
<p><small>{category} · {difficulty} · {input_type}</small></p>
<p><strong>{question}</strong></p>
<form method="post" action="/answer">
  <input type="hidden" name="correct_answer" value="{correct}">
  {options}
  <p>
    <button type="submit">Submit answer</button>
    <a href="/" style="margin-left:1rem;">New question</a>
  </p>
</form>"#,
        ),
    )
}

#[derive(Debug, Deserialize)]
struct AnswerForm {
    answer: String,
    correct_answer: String,
}

async fn check_answer(Form(form): Form<AnswerForm>) -> impl IntoResponse {
    let correct = form.correct_answer.trim().eq_ignore_ascii_case(form.answer.trim());

    let message = if correct {
        "Correct! Nice work."
    } else {
        "Not quite."
    };

    let body = format!(
        r#"<h1>{message}</h1>
<p>Your answer: <strong>{yours}</strong></p>
<p>The correct answer was: <strong>{correct}</strong></p>
<p><a href="/">Play another question</a></p>"#,
        message = message,
        yours = escape_html(&form.answer),
        correct = escape_html(&form.correct_answer),
    );

    layout("Result", &body)
}
