use crate::core::events::{Branch, GitEvent, PullRequest, Tag};
use crate::core::git_service::{self, GitServiceError};
use crate::infrastructure::db::{self, DbError, DbPool, Repository};
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use teloxide::prelude::*;
use teloxide::types::ParseMode;
use teloxide::utils::markdown::escape;
use teloxide::RequestError;

pub async fn run_updater(bot: Bot, pool: DbPool) {
    let mut update_interval = tokio::time::interval(Duration::from_secs(60));
    let mut cleanup_interval = tokio::time::interval(Duration::from_secs(3600));

    loop {
        tokio::select! {
            _ = update_interval.tick() => {
                log::info!("Running repository update check...");
                if let Err(e) = check_for_updates(&bot, &pool).await {
                    log::error!("Error during repository update check: {:?}", e);
                }
            }
            _ = cleanup_interval.tick() => {
                log::info!("Running database cleanup...");
                if let Err(e) = cleanup_database(&pool).await {
                    log::error!("Error during database cleanup: {:?}", e);
                }
            }
        }
    }
}

pub async fn cleanup_database(
    pool: &DbPool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let repos_affected = db::remove_orphan_repositories(pool).await?;
    if repos_affected > 0 {
        log::info!("Removed {} orphan repositories.", repos_affected);
    }

    let users_affected = db::remove_orphan_users(pool).await?;
    if users_affected > 0 {
        log::info!("Removed {} orphan users.", users_affected);
    }

    Ok(())
}

pub async fn check_for_updates(
    bot: &Bot,
    pool: &DbPool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let repos = db::get_all_repositories(pool).await?;

    for repo in &repos {
        log::debug!("Checking repo: {}", repo.url);
        let remote_refs = match git_service::ls_remote(&repo.url).await {
            Ok(refs) => refs,
            Err(e) => {
                if let GitServiceError::Git(git_err) = &e {
                    if git_err.class() == git2::ErrorClass::Http
                        && (git_err.code() == git2::ErrorCode::Auth
                            || git_err.code() == git2::ErrorCode::NotFound)
                    {
                        log::warn!(
                            "Repository {} is inaccessible (private or deleted). Removing.",
                            repo.url
                        );
                        handle_inaccessible_repository(bot, pool, &repo).await?;
                    }
                }
                log::error!("Failed to ls-remote for {}: {:?}", repo.url, e);
                continue;
            }
        };

        let db_refs = db::get_repository_refs(pool, repo.id).await?;
        let events = detect_events(&remote_refs, &db_refs);

        if !events.is_empty() {
            for event in &events {
                log::info!("Update detected for {}: {:?}", repo.url, event);
                update_database_from_event(pool, repo.id, event).await?;
                notify_subscribers(bot, pool, repo.id, &repo.url, event).await?;
            }
        }

        let deleted_refs = detect_deleted_refs(&remote_refs, &db_refs);
        if !deleted_refs.is_empty() {
            log::info!("Detected {} deleted refs for {}", deleted_refs.len(), repo.url);
            for ref_name in deleted_refs {
                db::delete_ref(pool, repo.id, &ref_name).await?;
            }
        }
    }
    Ok(())
}

fn detect_events(
    remote_refs: &HashMap<String, String>,
    db_refs: &HashMap<String, String>,
) -> Vec<GitEvent> {
    let mut events = Vec::new();

    for (ref_name, new_sha) in remote_refs {
        let event = match db_refs.get(ref_name) {
            Some(old_sha) if old_sha == new_sha => None,
            Some(old_sha) => {
                if ref_name.starts_with("refs/heads/") {
                    Some(GitEvent::BranchUpdated {
                        name: ref_name.clone(),
                        old_sha: old_sha.clone(),
                        new_sha: new_sha.clone(),
                    })
                } else if ref_name.starts_with("refs/pull/") {
                    ref_name.split('/').nth(2).and_then(|id| id.parse().ok()).map(|pr_id| {
                        GitEvent::PullRequestUpdated(PullRequest {
                            id: pr_id,
                            sha: new_sha.clone(),
                        })
                    })
                } else {
                    None
                }
            }
            None => {
                if ref_name.starts_with("refs/heads/") {
                    Some(GitEvent::NewBranch(Branch {
                        name: ref_name.clone(),
                        sha: new_sha.clone(),
                    }))
                } else if ref_name.starts_with("refs/tags/") {
                    Some(GitEvent::NewTag(Tag {
                        name: ref_name.clone(),
                        sha: new_sha.clone(),
                    }))
                } else if ref_name.starts_with("refs/pull/") {
                    ref_name.split('/').nth(2).and_then(|id| id.parse().ok()).map(|pr_id| {
                        GitEvent::NewPullRequest(PullRequest {
                            id: pr_id,
                            sha: new_sha.clone(),
                        })
                    })
                } else {
                    None
                }
            }
        };
        if let Some(event) = event {
            events.push(event);
        }
    }
    events
}

fn detect_deleted_refs(
    remote_refs: &HashMap<String, String>,
    db_refs: &HashMap<String, String>,
) -> HashSet<String> {
    let remote_keys: HashSet<_> = remote_refs.keys().cloned().collect();
    let db_keys: HashSet<_> = db_refs.keys().cloned().collect();
    db_keys.difference(&remote_keys).cloned().collect()
}

async fn update_database_from_event(
    pool: &DbPool,
    repo_id: i32,
    event: &GitEvent,
) -> Result<(), DbError> {
    match event {
        GitEvent::NewBranch(branch) => {
            db::update_ref_hash(pool, repo_id, &branch.name, &branch.sha).await
        }
        GitEvent::NewTag(tag) => db::update_ref_hash(pool, repo_id, &tag.name, &tag.sha).await,
        GitEvent::BranchUpdated { name, new_sha, .. } => {
            db::update_ref_hash(pool, repo_id, name, new_sha).await
        }
        GitEvent::NewPullRequest(pr) => {
            let ref_name = format!("refs/pull/{}/head", pr.id);
            db::update_ref_hash(pool, repo_id, &ref_name, &pr.sha).await
        }
        GitEvent::PullRequestUpdated(pr) => {
            let ref_name = format!("refs/pull/{}/head", pr.id);
            db::update_ref_hash(pool, repo_id, &ref_name, &pr.sha).await
        }
        GitEvent::NoChanges => Ok(()),
    }
}

async fn handle_inaccessible_repository(
    bot: &Bot,
    pool: &DbPool,
    repo: &Repository,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let message = format!(
        "⚠️ Repository [{}]({}) is no longer accessible (it may have been deleted or made private). You have been unsubscribed.",
        escape(&repo.url),
        escape(&repo.url)
    );

    let subscribers = db::get_subscribers_with_settings(pool, repo.id).await?;
    for (chat_id, _) in subscribers {
        if let Err(e) = bot
            .send_message(chat_id, &message)
            .disable_web_page_preview(true)
            .parse_mode(ParseMode::MarkdownV2)
            .await
        {
            if let RequestError::Api(teloxide::ApiError::BotBlocked) = e {
                log::warn!("User {} has blocked the bot. Removing user.", chat_id);
                db::remove_user(pool, chat_id.0).await?;
            } else {
                log::error!(
                    "Failed to send inaccessible repo notification to {}: {:?}",
                    chat_id,
                    e
                );
            }
        }
    }

    db::remove_repository(pool, repo.id).await?;
    Ok(())
}

fn format_notification_message(repo_url: &str, event: &GitEvent) -> String {
    let base_url = repo_url.trim_end_matches(".git");
    let short_repo_name = base_url
        .split('/')
        .rev()
        .take(2)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("/");

    let rendered_event = event.render_as_notification().unwrap_or_default();

    let details = match event {
        GitEvent::NewBranch(branch) => {
            let short_ref = branch.name.trim_start_matches("refs/heads/");
            let commit_hash_short = &branch.sha[..7];
            let commit_url = format!("{}/commit/{}", base_url, branch.sha);
            let ref_url = format!("{}/tree/{}", base_url, short_ref);
            format!(
                "Branch: [{}]({})\nCommit: [{}]({})",
                escape(short_ref),
                escape(&ref_url),
                escape(commit_hash_short),
                escape(&commit_url)
            )
        }
        GitEvent::NewTag(tag) => {
            let short_ref = tag.name.trim_start_matches("refs/tags/");
            let commit_hash_short = &tag.sha[..7];
            let commit_url = format!("{}/commit/{}", base_url, tag.sha);
            let ref_url = format!("{}/releases/tag/{}", base_url, short_ref);
            format!(
                "Tag: [{}]({})\nCommit: [{}]({})",
                escape(short_ref),
                escape(&ref_url),
                escape(commit_hash_short),
                escape(&commit_url)
            )
        }
        GitEvent::BranchUpdated {
            name,
            old_sha,
            new_sha,
        } => {
            let short_ref = name.trim_start_matches("refs/heads/");
            let compare_url = format!("{}/compare/{}...{}", base_url, old_sha, new_sha);
            format!(
                "Branch: [{}]({}/tree/{})\nChanges: [compare]({})",
                escape(short_ref),
                escape(base_url),
                escape(short_ref),
                escape(&compare_url)
            )
        }
        GitEvent::NewPullRequest(pr) => {
            format!("Pull Request: [\\#{}](_)", pr.id)
        }
        GitEvent::PullRequestUpdated(pr) => {
            format!("Pull Request: [\\#{}](_)", pr.id)
        }
        GitEvent::NoChanges => "".to_string(),
    };

    format!(
        "{}\nRepository: [{}]({})\n{}",
        rendered_event,
        escape(&short_repo_name),
        escape(base_url),
        details
    )
}

async fn notify_subscribers(
    bot: &Bot,
    pool: &DbPool,
    repo_id: i32,
    repo_url: &str,
    event: &GitEvent,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let subscribers = db::get_subscribers_with_settings(pool, repo_id).await?;
    let message = format_notification_message(repo_url, event);

    for (chat_id, settings) in subscribers {
        let should_notify = match event {
            GitEvent::NewBranch(_) => settings.notify_on_new_branch,
            GitEvent::NewTag(_) => settings.notify_on_new_tag,
            GitEvent::BranchUpdated { .. } => settings.notify_on_branch_update,
            GitEvent::NewPullRequest(_) => settings.notify_on_new_pr,
            GitEvent::PullRequestUpdated(_) => settings.notify_on_pr_update,
            GitEvent::NoChanges => false,
        };

        if !should_notify {
            continue;
        }

        if let Err(e) = bot
            .send_message(chat_id, &message)
            .parse_mode(ParseMode::MarkdownV2)
            .disable_web_page_preview(true)
            .await
        {
            if let RequestError::Api(teloxide::ApiError::BotBlocked) = e {
                log::warn!("User {} has blocked the bot. Removing user.", chat_id);
                db::remove_user(pool, chat_id.0).await?;
            } else {
                log::error!("Failed to send notification to {}: {:?}", chat_id, e);
            }
        }
    }

    Ok(())
}
