use std::collections::HashMap;
use thiserror::Error;
use tokio::task;

#[derive(Debug, Error)]
pub enum GitServiceError {
    #[error("Git operation failed: {0}")]
    Git(#[from] git2::Error),
    #[error("Internal task execution error")]
    Task,
}

pub async fn ls_remote(url: &str) -> Result<HashMap<String, String>, GitServiceError> {
    let url_owned = url.to_string();
    task::spawn_blocking(move || {
        let mut remote = git2::Remote::create_detached(url_owned.as_bytes())?;
        remote.connect(git2::Direction::Fetch)?;
        let list = remote.list()?;
        let refs = list
            .iter()
            .filter(|head| {
                let name = head.name();
                name.starts_with("refs/heads/")
                    || name.starts_with("refs/tags/")
                    || (name.starts_with("refs/pull/") && name.ends_with("/head"))
            })
            .map(|head| (head.name().to_string(), head.oid().to_string()))
            .collect();
        Ok(refs)
    })
    .await
    .map_err(|_| GitServiceError::Task)?
}
