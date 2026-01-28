# GitNotify Telegram Bot

GitNotify is a sophisticated, menu-driven Telegram bot for monitoring Git repositories. It's built with Rust and leverages `teloxide`, `sqlx`, and `git2` to provide real-time notifications about repository events.

## Features

- **Real-Time Git Monitoring**: Uses an efficient `git ls-remote` mechanism to detect changes in branches, tags, and pull requests without cloning the entire repository.
- **Menu-Driven Interface**: All interactions are handled through clean, intuitive inline keyboard menus. No manual command typing required.
- **Clean Chat Experience**: The bot edits its own messages to display new menus, creating a seamless, single-page application feel and preventing chat spam.
- **Granular Notification Control**:
    - **Per-Repository Settings**: Fine-tune which notifications you want to receive for each repository (e.g., only new tags and pull requests).
    - **Global Toggle**: Instantly mute or unmute all notifications with a single command.
- **Structured Logging**: All operations are logged in a structured JSON format, ready for easy parsing and analysis.
- **Clean Architecture**: The project follows a clean, layered architecture, making it scalable and easy to maintain.

## Technical Stack

- **Language**: Rust (Stable)
- **Telegram Bot Framework**: `teloxide`
- **Database**: MySQL with `sqlx` for asynchronous queries and connection pooling.
- **Git Operations**: `git2` (libgit2 bindings) for `ls-remote` functionality.
- **Logging**: `tracing` with `tracing-subscriber` for structured JSON logging.
- **Serialization**: `serde` and `serde_json`.

## Getting Started

### Prerequisites

- Rust toolchain (latest stable version)
- MySQL server
- A Telegram Bot Token

### Installation & Setup

1.  **Clone the repository:**
    ```bash
    git clone https://github.com/sleepyhead228/GitNotify.git
    cd GitNotify
    ```

2.  **Set up the database:**
    - Create a new MySQL database.
    - Run the schema script to create the necessary tables:
      ```bash
      mysql -u your_user -p your_database < migrations/initial_schema.sql
      ```

3.  **Configure environment variables:**
    - Create a `.env` file in the project root by copying the example:
      ```bash
      cp .env.example .env
      ```
    - Edit the `.env` file with your credentials:
      ```env
      TELOXIDE_TOKEN=your_telegram_bot_token
      DATABASE_URL=mysql://user:password@host:port/database
      RUST_LOG=info
      ```

4.  **Build and run the bot:**
    ```bash
    cargo run --release
    ```

The bot will start, and you can interact with it on Telegram!

## Architecture

The project is organized into a clean, layered architecture:

-   `src/main.rs`: Entry point, dispatcher setup, and top-level handlers.
-   `src/bot/`: Contains UI generation (`ui.rs`) and dialogue state management (`dialogue.rs`).
-   `src/core/`: Core application logic, including the `updater` service, `git_service`, and `events` definitions.
-   `src/infrastructure/`: Handles external concerns like database connections (`db.rs`) and logging (`logging.rs`).


