# Руководство по установке

## Зависимости

Для работы **GitNotify** необходимы следующие компоненты:

*   [Rust](https://www.rust-lang.org/tools/install) 
*   [Git](https://git-scm.com/downloads)
*   [MySQL](https://www.mysql.com/downloads/) 

## Установка

1.  **Клонируйте репозиторий:**

    ```bash
    git clone https://github.com/sleepyhead228/GitNotify.git
    cd GitNotify
    ```

2.  **Настройте базу данных:**

    Установите MySQL. Для создания структуры базы данных выполните SQL-скрипт из репозитория:

    ```bash
    mysql -u your_user -p your_database_name < migrations/initial_schema.sql
    ```
    > Замените `your_user` и `your_database_name` на ваши данные.

3.  **Настройте конфигурационный файл:**

    Скопируйте `.env.example` в `.env`. Укажите в файле `.env` данные для подключения к базе данных, а также токен бота.

    ```bash
    cp .env.example .env
    ```

    Пример переменной `DATABASE_URL` для `.env`:
    ```
    DATABASE_URL=mysql://user:password@localhost:3306/gitnotify
    ```

4.  **Соберите и запустите проект:**

    ```bash
    cargo run --release
    ```
