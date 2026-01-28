use sha2::{Digest, Sha256};
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use std::collections::HashMap;
use std::env;
use teloxide::types::{ChatId, User};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("Database configuration error: {0}")]
    Config(#[from] env::VarError),
    #[error("Database query failed: {0}")]
    Query(#[from] sqlx::Error),
}

pub type DbPool = MySqlPool;

#[derive(Clone, sqlx::FromRow)]
pub struct Repository {
    pub id: i32,
    pub url: String,
}

#[derive(Clone, Debug, Default, sqlx::FromRow)]
pub struct SubscriptionSettings {
    #[sqlx(default)]
    pub notify_on_new_branch: bool,
    #[sqlx(default)]
    pub notify_on_new_tag: bool,
    #[sqlx(default)]
    pub notify_on_branch_update: bool,
    #[sqlx(default)]
    pub notify_on_new_pr: bool,
    #[sqlx(default)]
    pub notify_on_pr_update: bool,
}

pub async fn create_pool() -> Result<DbPool, DbError> {
    let database_url = env::var("DATABASE_URL")?;
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    Ok(pool)
}

pub async fn ensure_user_exists(pool: &DbPool, user: &User) -> Result<(), DbError> {
    sqlx::query!(
        "INSERT IGNORE INTO users (id, username) VALUES (?, ?)",
        user.id.0,
        user.username
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn add_repository_subscription(
    pool: &DbPool,
    user: &User,
    repo_url: &str,
) -> Result<(), DbError> {
    ensure_user_exists(pool, user).await?;

    let mut tx = pool.begin().await?;

    let url_hash = format!("{:x}", Sha256::digest(repo_url.as_bytes()));

    let repo_id = sqlx::query!(
        "INSERT IGNORE INTO repositories (url, url_hash) VALUES (?, ?)",
        repo_url,
        url_hash
    )
    .execute(&mut *tx)
    .await?
    .last_insert_id();

    let repo_id = if repo_id == 0 {
        sqlx::query!("SELECT id FROM repositories WHERE url_hash = ?", url_hash)
            .fetch_one(&mut *tx)
            .await?
            .id
    } else {
        repo_id as i32
    };

    sqlx::query!(
        "INSERT IGNORE INTO subscriptions (user_id, repository_id) VALUES (?, ?)",
        user.id.0,
        repo_id
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn remove_repository_subscription(
    pool: &DbPool,
    user_id: i64,
    repo_id: i32,
) -> Result<(), DbError> {
    sqlx::query!(
        "DELETE FROM subscriptions WHERE user_id = ? AND repository_id = ?",
        user_id,
        repo_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn remove_orphan_repositories(pool: &DbPool) -> Result<u64, DbError> {
    let result = sqlx::query("DELETE FROM repositories WHERE id NOT IN (SELECT DISTINCT repository_id FROM subscriptions)")
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

pub async fn remove_orphan_users(pool: &DbPool) -> Result<u64, DbError> {
    let result = sqlx::query(
        "DELETE FROM users WHERE id NOT IN (SELECT DISTINCT user_id FROM subscriptions)",
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

pub async fn remove_repository(pool: &DbPool, repo_id: i32) -> Result<(), DbError> {
    sqlx::query!("DELETE FROM repositories WHERE id = ?", repo_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn remove_user(pool: &DbPool, user_id: i64) -> Result<(), DbError> {
    sqlx::query!("DELETE FROM users WHERE id = ?", user_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_repository_by_id(
    pool: &DbPool,
    repo_id: i32,
) -> Result<Option<Repository>, DbError> {
    let repo = sqlx::query_as::<_, Repository>("SELECT id, url FROM repositories WHERE id = ?")
        .bind(repo_id)
        .fetch_optional(pool)
        .await?;
    Ok(repo)
}

pub async fn get_all_repositories(pool: &DbPool) -> Result<Vec<Repository>, DbError> {
    let repos = sqlx::query_as::<_, Repository>("SELECT id, url FROM repositories")
        .fetch_all(pool)
        .await?;
    Ok(repos)
}

pub async fn get_user_subscriptions(
    pool: &DbPool,
    user_id: i64,
) -> Result<Vec<Repository>, DbError> {
    let repos = sqlx::query_as::<_, Repository>(
        "SELECT r.id, r.url FROM repositories r
         JOIN subscriptions s ON r.id = s.repository_id
         WHERE s.user_id = ?",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(repos)
}

pub async fn get_repository_refs(
    pool: &DbPool,
    repo_id: i32,
) -> Result<HashMap<String, String>, DbError> {
    let refs = sqlx::query!(
        "SELECT ref_name, last_hash FROM repository_refs WHERE repository_id = ?",
        repo_id
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|rec| (rec.ref_name, rec.last_hash))
    .collect();
    Ok(refs)
}

pub async fn update_ref_hash(
    pool: &DbPool,
    repo_id: i32,
    ref_name: &str,
    new_hash: &str,
) -> Result<(), DbError> {
    sqlx::query!(
        "INSERT INTO repository_refs (repository_id, ref_name, last_hash) VALUES (?, ?, ?)
         ON DUPLICATE KEY UPDATE last_hash = VALUES(last_hash)",
        repo_id,
        ref_name,
        new_hash
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_ref(pool: &DbPool, repo_id: i32, ref_name: &str) -> Result<(), DbError> {
    sqlx::query!(
        "DELETE FROM repository_refs WHERE repository_id = ? AND ref_name = ?",
        repo_id,
        ref_name
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_subscribers_with_settings(
    pool: &DbPool,
    repo_id: i32,
) -> Result<HashMap<ChatId, SubscriptionSettings>, DbError> {
    let records = sqlx::query!(
        r#"
        SELECT
            u.id,
            s.notify_on_new_branch,
            s.notify_on_new_tag,
            s.notify_on_branch_update,
            s.notify_on_new_pr,
            s.notify_on_pr_update
        FROM subscriptions s
        JOIN users u ON s.user_id = u.id
        WHERE s.repository_id = ? AND u.notifications_enabled = TRUE
        "#,
        repo_id
    )
    .fetch_all(pool)
    .await?;

    let mut subscribers = HashMap::new();
    for record in records {
        let settings = SubscriptionSettings {
            notify_on_new_branch: record.notify_on_new_branch == 1,
            notify_on_new_tag: record.notify_on_new_tag == 1,
            notify_on_branch_update: record.notify_on_branch_update == 1,
            notify_on_new_pr: record.notify_on_new_pr == 1,
            notify_on_pr_update: record.notify_on_pr_update == 1,
        };
        subscribers.insert(ChatId(record.id), settings);
    }
    Ok(subscribers)
}

pub async fn get_user_notification_status(pool: &DbPool, user_id: i64) -> Result<bool, DbError> {
    let result = sqlx::query!(
        "SELECT notifications_enabled FROM users WHERE id = ?",
        user_id
    )
    .fetch_one(pool)
    .await?;
    Ok(result.notifications_enabled == 1)
}

pub async fn set_user_notification_status(
    pool: &DbPool,
    user_id: i64,
    status: bool,
) -> Result<(), DbError> {
    sqlx::query!(
        "UPDATE users SET notifications_enabled = ? WHERE id = ?",
        status,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_subscription_settings(
    pool: &DbPool,
    user_id: i64,
    repo_id: i32,
) -> Result<SubscriptionSettings, DbError> {
    let record = sqlx::query!(
        r#"
        SELECT
            notify_on_new_branch,
            notify_on_new_tag,
            notify_on_branch_update,
            notify_on_new_pr,
            notify_on_pr_update
        FROM subscriptions
        WHERE user_id = ? AND repository_id = ?
        "#,
        user_id,
        repo_id
    )
    .fetch_one(pool)
    .await?;

    Ok(SubscriptionSettings {
        notify_on_new_branch: record.notify_on_new_branch == 1,
        notify_on_new_tag: record.notify_on_new_tag == 1,
        notify_on_branch_update: record.notify_on_branch_update == 1,
        notify_on_new_pr: record.notify_on_new_pr == 1,
        notify_on_pr_update: record.notify_on_pr_update == 1,
    })
}

pub async fn update_subscription_settings(
    pool: &DbPool,
    user_id: i64,
    repo_id: i32,
    settings: &SubscriptionSettings,
) -> Result<(), DbError> {
    sqlx::query!(
        "UPDATE subscriptions
         SET notify_on_new_branch = ?, notify_on_new_tag = ?, notify_on_branch_update = ?, notify_on_new_pr = ?, notify_on_pr_update = ?
         WHERE user_id = ? AND repository_id = ?",
        settings.notify_on_new_branch,
        settings.notify_on_new_tag,
        settings.notify_on_branch_update,
        settings.notify_on_new_pr,
        settings.notify_on_pr_update,
        user_id,
        repo_id
    )
    .execute(pool)
    .await?;
    Ok(())
}
