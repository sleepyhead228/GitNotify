use serde::{Deserialize, Serialize};
use teloxide::utils::markdown::escape;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Branch {
    pub name: String,
    pub sha: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Tag {
    pub name: String,
    pub sha: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PullRequest {
    pub id: u64,
    pub sha: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum GitEvent {
    NewBranch(Branch),
    NewTag(Tag),
    BranchUpdated {
        name: String,
        old_sha: String,
        new_sha: String,
    },
    NewPullRequest(PullRequest),
    PullRequestUpdated(PullRequest),
    NoChanges,
}

impl GitEvent {
    pub fn render_as_notification(&self) -> Option<String> {
        match self {
            GitEvent::NewBranch(branch) => {
                let branch_name = branch.name.trim_start_matches("refs/heads/");
                Some(format!("🌿 New Branch: *{}*", escape(branch_name)))
            }
            GitEvent::NewTag(tag) => {
                let tag_name = tag.name.trim_start_matches("refs/tags/");
                Some(format!("🏷️ New Tag: *{}*", escape(tag_name)))
            }
            GitEvent::BranchUpdated { name, .. } => {
                let branch_name = name.trim_start_matches("refs/heads/");
                Some(format!("🚀 Branch Updated: *{}*", escape(branch_name)))
            }
            GitEvent::NewPullRequest(pr) => Some(format!("📦 New Pull Request: *\\#{}*", pr.id)),
            GitEvent::PullRequestUpdated(pr) => {
                Some(format!("📥 Pull Request Updated: *\\#{}*", pr.id))
            }
            GitEvent::NoChanges => None,
        }
    }
}
