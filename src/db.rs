use sqlx::{FromRow, SqlitePool};

#[derive(Debug, FromRow)]
pub struct Player {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, FromRow)]
pub struct AttemptWithPlayer {
    pub player_name: String,
    pub question: String,
    pub player_answer: String,
    pub correct_answer: String,
    pub correct: i32,
    pub answered_at: String,
}

#[derive(Debug)]
pub struct PlayerStats {
    pub total: i64,
    pub correct: i64,
}

pub async fn create_player(pool: &SqlitePool, name: &str) -> sqlx::Result<i64> {
    let result = sqlx::query("INSERT INTO players (name) VALUES (?)")
        .bind(name.trim())
        .execute(pool)
        .await?;
    Ok(result.last_insert_rowid())
}

pub async fn get_player(pool: &SqlitePool, id: i64) -> sqlx::Result<Option<Player>> {
    sqlx::query_as::<_, Player>("SELECT id, name FROM players WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn record_attempt(
    pool: &SqlitePool,
    player_id: i64,
    question: &str,
    player_answer: &str,
    correct_answer: &str,
    correct: bool,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO attempts (player_id, question, player_answer, correct_answer, correct) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(player_id)
    .bind(question)
    .bind(player_answer)
    .bind(correct_answer)
    .bind(i32::from(correct))
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn all_attempts(pool: &SqlitePool) -> sqlx::Result<Vec<AttemptWithPlayer>> {
    sqlx::query_as::<_, AttemptWithPlayer>(
        r#"
        SELECT
            p.name AS player_name,
            a.question,
            a.player_answer,
            a.correct_answer,
            a.correct,
            a.answered_at
        FROM attempts a
        INNER JOIN players p ON p.id = a.player_id
        ORDER BY datetime(a.answered_at) DESC
        "#,
    )
    .fetch_all(pool)
    .await
}

pub async fn player_stats(pool: &SqlitePool, player_id: i64) -> sqlx::Result<PlayerStats> {
    let row: (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(correct), 0) FROM attempts WHERE player_id = ?",
    )
    .bind(player_id)
    .fetch_one(pool)
    .await?;
    Ok(PlayerStats {
        total: row.0,
        correct: row.1,
    })
}
