CREATE DATABASE IF NOT EXISTS gitnofity;
USE gitnofity;

CREATE TABLE IF NOT EXISTS users (
    id BIGINT PRIMARY KEY,
    username VARCHAR(255),
    notifications_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS repositories (
    id INT AUTO_INCREMENT PRIMARY KEY,
    url VARCHAR(2048) NOT NULL,
    url_hash VARCHAR(64) NOT NULL UNIQUE,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS subscriptions (
    user_id BIGINT NOT NULL,
    repository_id INT NOT NULL,
    notify_on_new_branch BOOLEAN NOT NULL DEFAULT TRUE,
    notify_on_new_tag BOOLEAN NOT NULL DEFAULT TRUE,
    notify_on_branch_update BOOLEAN NOT NULL DEFAULT TRUE,
    notify_on_new_pr BOOLEAN NOT NULL DEFAULT TRUE,
    notify_on_pr_update BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (user_id, repository_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (repository_id) REFERENCES repositories(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS repository_refs (
    id INT AUTO_INCREMENT PRIMARY KEY,
    repository_id INT NOT NULL,
    ref_name VARCHAR(255) NOT NULL,
    last_hash VARCHAR(64) NOT NULL,
    last_updated TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY (repository_id, ref_name),
    FOREIGN KEY (repository_id) REFERENCES repositories(id) ON DELETE CASCADE
);
