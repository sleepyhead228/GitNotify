mod bot;
mod core;
mod infrastructure;

use crate::bot::dialogue::{Dialogue, InMemStorage, State};
use crate::bot::ui::{global_notification_toggle_menu, notification_settings_menu, repository_menu, subscriptions_menu};
use crate::core::updater;
use crate::infrastructure::db::{self, DbPool};
use crate::infrastructure::logging::init_logging;
use anyhow::anyhow;
use dotenv::dotenv;
use teloxide::dptree;
use teloxide::prelude::*;
use teloxide::types::{MessageId, ParseMode};
use teloxide::utils::command::BotCommands;
use teloxide::utils::markdown::escape;
use teloxide::RequestError;

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "These commands are supported:")]
enum Command {
    #[command(description = "List your subscriptions.")]
    ListRepos,
    #[command(description = "Add a new repository subscription.")]
    AddRepo,
    #[command(description = "Toggle all notifications on/off.")]
    Toggle,
}

type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[tokio::main]
async fn main() {
    dotenv().ok();
    let _guard = init_logging();
    log::info!("Starting bot...");

    let bot = Bot::from_env();
    bot.set_my_commands(Command::bot_commands())
        .await
        .expect("Failed to set commands");

    let pool = db::create_pool().await.expect("Failed to create database pool");

    log::info!("Running initial database cleanup...");
    if let Err(e) = updater::cleanup_database(&pool).await {
        log::error!("Initial database cleanup failed: {:?}", e);
    }

    log::info!("Running initial repository update check...");
    if let Err(e) = updater::check_for_updates(&bot, &pool).await {
        log::error!("Initial repository update check failed: {:?}", e);
    }

    tokio::spawn(updater::run_updater(bot.clone(), pool.clone()));

    let message_handler_chain = Update::filter_message()
        .enter_dialogue::<Message, InMemStorage<State>, State>()
        .branch(dptree::filter(|msg: Message| msg.text().map_or(false, |text| text == "/start")).endpoint(start_handler))
        .branch(dptree::entry().filter_command::<Command>().endpoint(command_handler))
        .branch(dptree::entry().endpoint(message_handler));

    let callback_handler_chain = Update::filter_callback_query()
        .enter_dialogue::<CallbackQuery, InMemStorage<State>, State>()
        .endpoint(callback_handler);

    let schema = dptree::entry()
        .branch(message_handler_chain)
        .branch(callback_handler_chain);

    Dispatcher::builder(bot, schema)
        .dependencies(dptree::deps![InMemStorage::<State>::new(), pool])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}

async fn start_handler(bot: Bot, dialogue: Dialogue, msg: Message, pool: DbPool) -> HandlerResult {
    let user = msg.from().ok_or_else(|| anyhow!("Message has no sender"))?;
    db::ensure_user_exists(&pool, user).await?;
    dialogue.update(State::Start).await?;
    bot.send_message(msg.chat.id, "👋 Welcome to GitNotify! Use the menu to manage your repository subscriptions.").await?;
    Ok(())
}

async fn command_handler(bot: Bot, dialogue: Dialogue, msg: Message, cmd: Command, pool: DbPool) -> HandlerResult {
    let user = msg.from().ok_or_else(|| anyhow!("Message has no sender"))?;
    db::ensure_user_exists(&pool, user).await?;
    match cmd {
        Command::ListRepos => {
            send_subscriptions_list(bot, msg.chat.id, None, &pool).await?;
        }
        Command::AddRepo => {
            dialogue.update(State::ReceiveRepoUrl).await?;
            bot.send_message(msg.chat.id, "🔗 Send me the repository URL (e.g., https://github.com/user/repo)")
                .disable_web_page_preview(true)
                .await?;
        }
        Command::Toggle => {
            let is_enabled = db::get_user_notification_status(&pool, msg.chat.id.0).await?;
            let text = if is_enabled {
                "Globally enabling all notifications."
            } else {
                "Globally disabling all notifications."
            };
            bot.send_message(msg.chat.id, text)
                .reply_markup(global_notification_toggle_menu(is_enabled))
                .await?;
        }
    }
    Ok(())
}

async fn callback_handler(bot: Bot, _dialogue: Dialogue, q: CallbackQuery, pool: DbPool) -> HandlerResult {
    db::ensure_user_exists(&pool, &q.from).await?;
    let msg = q.message.ok_or_else(|| anyhow!("Callback query has no message"))?;

    if let Some(data) = q.data {
        let result: Result<(), Box<dyn std::error::Error + Send + Sync>> = match data.as_str() {
            "list_repos" => {
                send_subscriptions_list(bot.clone(), msg.chat.id, Some(msg.id), &pool).await?;
                Ok(())
            }
            _ if data.starts_with("view_repo_") => {
                let repo_id: i32 = data.trim_start_matches("view_repo_").parse()?;
                let repo = db::get_repository_by_id(&pool, repo_id).await?.ok_or_else(|| anyhow!("Repository not found"))?;
                let refs = db::get_repository_refs(&pool, repo_id).await?;

                let base_url = repo.url.trim_end_matches(".git");
                let short_repo_name = base_url.split('/').rev().take(2).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("/");

                let mut text = format!("📦 *Repository:* [{}]({})\n\n", escape(&short_repo_name), escape(base_url));
                text.push_str("*Tracked references:*\n");

                let mut sorted_refs: Vec<_> = refs.into_iter().collect();
                sorted_refs.sort_by(|a, b| a.0.cmp(&b.0));

                const MAX_REFS_DISPLAY: usize = 10;
                let mut displayed_count = 0;

                if sorted_refs.is_empty() {
                    text.push_str("  _None yet._");
                } else {
                    for (ref_name, hash) in sorted_refs.iter() {
                        if displayed_count >= MAX_REFS_DISPLAY {
                            text.push_str(&format!("  _...and {} more references._\n", sorted_refs.len() - displayed_count));
                            break;
                        }

                        let display_ref_name = ref_name.trim_start_matches("refs/heads/").trim_start_matches("refs/tags/").trim_start_matches("refs/pull/");
                        let ref_link = if ref_name.starts_with("refs/heads/") {
                            format!("{}/tree/{}", base_url, display_ref_name)
                        } else if ref_name.starts_with("refs/tags/") {
                            format!("{}/releases/tag/{}", base_url, display_ref_name)
                        } else if ref_name.starts_with("refs/pull/") {
                            format!("{}/pull/{}", base_url, display_ref_name)
                        } else {
                            base_url.to_string()
                        };
                        let commit_link = format!("{}/commit/{}", base_url, hash);
                        text.push_str(&format!("  • [{}]({}): [{}]({})\n", escape(display_ref_name), escape(&ref_link), &escape(&hash[..7]), escape(&commit_link)));
                        displayed_count += 1;
                    }
                }

                bot.edit_message_text(msg.chat.id, msg.id, text)
                    .disable_web_page_preview(true)
                    .reply_markup(repository_menu(repo_id))
                    .parse_mode(ParseMode::MarkdownV2)
                    .await?;
                Ok(())
            }
            _ if data.starts_with("unsubscribe_") => {
                let repo_id: i32 = data.trim_start_matches("unsubscribe_").parse()?;
                db::remove_repository_subscription(&pool, msg.chat.id.0, repo_id).await?;
                send_subscriptions_list(bot.clone(), msg.chat.id, Some(msg.id), &pool).await?;
                Ok(())
            }
            _ if data.starts_with("repo_settings_") => {
                let repo_id: i32 = data.trim_start_matches("repo_settings_").parse()?;
                let settings = db::get_subscription_settings(&pool, msg.chat.id.0, repo_id).await?;
                bot.edit_message_text(msg.chat.id, msg.id, "⚙️ Configure notifications for this repository:")
                    .reply_markup(notification_settings_menu(repo_id, &settings))
                    .await?;
                Ok(())
            }
            _ if data.starts_with("toggle_setting_") => {
                let parts: Vec<&str> = data.split('_').collect();
                let repo_id: i32 = parts[2].parse()?;
                let setting_name = parts[3..].join("_");

                let mut settings = db::get_subscription_settings(&pool, msg.chat.id.0, repo_id).await?;

                match setting_name.as_str() {
                    "new_branch" => settings.notify_on_new_branch = !settings.notify_on_new_branch,
                    "new_tag" => settings.notify_on_new_tag = !settings.notify_on_new_tag,
                    "branch_update" => settings.notify_on_branch_update = !settings.notify_on_branch_update,
                    "new_pr" => settings.notify_on_new_pr = !settings.notify_on_new_pr,
                    "pr_update" => settings.notify_on_pr_update = !settings.notify_on_pr_update,
                    _ => log::warn!("Unknown setting name: {}", setting_name),
                }

                db::update_subscription_settings(&pool, msg.chat.id.0, repo_id, &settings).await?;
                let updated_settings = db::get_subscription_settings(&pool, msg.chat.id.0, repo_id).await?;

                bot.edit_message_text(msg.chat.id, msg.id, "⚙️ Configure notifications for this repository:")
                    .reply_markup(notification_settings_menu(repo_id, &updated_settings))
                    .await?;
                Ok(())
            }
            "toggle_global_notifications" => {
                let current_status = db::get_user_notification_status(&pool, msg.chat.id.0).await?;
                let new_status = !current_status;
                db::set_user_notification_status(&pool, msg.chat.id.0, new_status).await?;
                let text = if new_status {
                    "✅ All notifications have been enabled."
                } else {
                    "❌ All notifications have been disabled."
                };
                bot.edit_message_text(msg.chat.id, msg.id, text)
                    .reply_markup(global_notification_toggle_menu(new_status))
                    .await?;
                Ok(())
            }
            _ => {
                bot.edit_message_text(msg.chat.id, msg.id, "❓ Unknown action. Please try again.").await?;
                Ok(())
            }
        };

        if let Err(e) = result {
            if let Some(req_err) = e.downcast_ref::<RequestError>() {
                if let RequestError::Api(teloxide::ApiError::MessageNotModified) = req_err {
                    log::debug!("Message not modified, ignoring.");
                } else {
                    return Err(e);
                }
            } else {
                return Err(e);
            }
        }
    }
    bot.answer_callback_query(q.id).await?;
    Ok(())
}

async fn message_handler(bot: Bot, dialogue: Dialogue, msg: Message, pool: DbPool) -> HandlerResult {
    let user = msg.from().ok_or_else(|| anyhow!("Message has no sender"))?;
    db::ensure_user_exists(&pool, user).await?;
    let state = dialogue.get().await?.unwrap_or_default();

    match state {
        State::ReceiveRepoUrl => {
            let url = msg.text().ok_or_else(|| anyhow!("Message has no text"))?;
            let status_msg = bot.send_message(msg.chat.id, "⏳ Checking repository...").disable_web_page_preview(true).await?;
            dialogue.update(State::Start).await?;

            match core::git_service::ls_remote(url).await {
                Ok(_) => {
                    match db::add_repository_subscription(&pool, user, url).await {
                        Ok(_) => {
                            bot.edit_message_text(status_msg.chat.id, status_msg.id, "✅ Successfully subscribed to the repository!").await?;
                        }
                        Err(e) => {
                            log::error!("Database error: {:?}", e);
                            bot.edit_message_text(status_msg.chat.id, status_msg.id, "❌ An internal error occurred while subscribing.").await?;
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Failed to ls_remote for {}: {:?}", url, e);
                    bot.edit_message_text(status_msg.chat.id, status_msg.id, "⚠️ Could not access the repository. Please check the URL and ensure the repository is public, then try again.").await?;
                }
            }
        }
        State::Start => {
            bot.send_message(msg.chat.id, "ℹ️ Please use the menu commands.").await?;
        }
    }

    Ok(())
}

async fn send_subscriptions_list(bot: Bot, chat_id: ChatId, message_id: Option<MessageId>, pool: &DbPool) -> HandlerResult {
    let subscriptions = db::get_user_subscriptions(pool, chat_id.0).await?;
    let text = if subscriptions.is_empty() {
        "📚 You have no active subscriptions."
    } else {
        "📚 Your current subscriptions:"
    };
    let markup = subscriptions_menu(&subscriptions);

    if let Some(mid) = message_id {
        bot.edit_message_text(chat_id, mid, text)
            .disable_web_page_preview(true)
            .reply_markup(markup)
            .await?;
    } else {
        bot.send_message(chat_id, text)
            .disable_web_page_preview(true)
            .reply_markup(markup)
            .await?;
    }
    Ok(())
}
