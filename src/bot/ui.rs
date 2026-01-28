use crate::infrastructure::db::{Repository, SubscriptionSettings};
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

pub fn subscriptions_menu(subscriptions: &[Repository]) -> InlineKeyboardMarkup {
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = vec![];

    for repo in subscriptions {
        let button_text = repo.url.split('/').last().unwrap_or(&repo.url);
        keyboard.push(vec![InlineKeyboardButton::callback(
            format!("📦 {}", button_text),
            format!("view_repo_{}", repo.id),
        )]);
    }

    keyboard.push(vec![InlineKeyboardButton::callback(
        "🔄 Refresh",
        "list_repos",
    )]);
    InlineKeyboardMarkup::new(keyboard)
}

pub fn repository_menu(repo_id: i32) -> InlineKeyboardMarkup {
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = vec![];
    keyboard.push(vec![InlineKeyboardButton::callback(
        "⚙️ Notification Settings",
        format!("repo_settings_{}", repo_id),
    )]);
    keyboard.push(vec![InlineKeyboardButton::callback(
        "❌ Unsubscribe",
        format!("unsubscribe_{}", repo_id),
    )]);
    keyboard.push(vec![InlineKeyboardButton::callback(
        "⬅️ Back to list",
        "list_repos",
    )]);
    InlineKeyboardMarkup::new(keyboard)
}

pub fn notification_settings_menu(
    repo_id: i32,
    settings: &SubscriptionSettings,
) -> InlineKeyboardMarkup {
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = vec![];

    let new_branch_text = if settings.notify_on_new_branch {
        "✅ New Branch"
    } else {
        "❌ New Branch"
    };
    keyboard.push(vec![InlineKeyboardButton::callback(
        new_branch_text,
        format!("toggle_setting_{}_new_branch", repo_id),
    )]);

    let new_tag_text = if settings.notify_on_new_tag {
        "✅ New Release (Tag)"
    } else {
        "❌ New Release (Tag)"
    };
    keyboard.push(vec![InlineKeyboardButton::callback(
        new_tag_text,
        format!("toggle_setting_{}_new_tag", repo_id),
    )]);

    let branch_update_text = if settings.notify_on_branch_update {
        "✅ Branch Updated"
    } else {
        "❌ Branch Updated"
    };
    keyboard.push(vec![InlineKeyboardButton::callback(
        branch_update_text,
        format!("toggle_setting_{}_branch_update", repo_id),
    )]);

    let new_pr_text = if settings.notify_on_new_pr {
        "✅ New Pull Request"
    } else {
        "❌ New Pull Request"
    };
    keyboard.push(vec![InlineKeyboardButton::callback(
        new_pr_text,
        format!("toggle_setting_{}_new_pr", repo_id),
    )]);

    let pr_update_text = if settings.notify_on_pr_update {
        "✅ Pull Request Updated"
    } else {
        "❌ Pull Request Updated"
    };
    keyboard.push(vec![InlineKeyboardButton::callback(
        pr_update_text,
        format!("toggle_setting_{}_pr_update", repo_id),
    )]);

    keyboard.push(vec![InlineKeyboardButton::callback(
        "⬅️ Back to Repository",
        format!("view_repo_{}", repo_id),
    )]);

    InlineKeyboardMarkup::new(keyboard)
}

pub fn global_notification_toggle_menu(is_enabled: bool) -> InlineKeyboardMarkup {
    let toggle_text = if is_enabled {
        "✅ All Notifications ON"
    } else {
        "❌ All Notifications OFF"
    };
    InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
        toggle_text,
        "toggle_global_notifications",
    )]])
}
