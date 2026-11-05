# 05 — 資料模型

完整 DDL：`schema.sql`（Alembic head `4d7c2a91b6e3`，由參考環境以 production 相同方式建立後 `pg_dump`）。

## 5.1 資料表概覽

| 資料表 | 用途 | 重要約束 |
| --- | --- | --- |
| `users` | 使用者 | `username` unique index；`is_super_user` 可為 NULL |
| `user_preferences` | 使用者偏好 | `user_id` unique、FK → users ON DELETE CASCADE；兩個布林 NOT NULL **無 DB default** |
| `users_departments` | 系所成員/加入申請 | FK user/department ON DELETE CASCADE；`status` DB default `'PENDING'`；**無** `(user_id, department_id)` 唯一約束 |
| `departments` | 系所 | `sort_order` NOT NULL DEFAULT 999 |
| `courses` | 課程 | `department_id` **無 FK** |
| `posts` | 考古題 | `owner_id`/`course_id`/`department_id` **無 FK** |
| `posts_files` | 考古題檔案 URL | `post_id` **無 FK** |
| `bulletins` | 公告 | `department_id` **無 FK** |
| `threads` | 討論串 | FK owner → users、course → courses（皆無 cascade） |
| `thread_comments` | 留言（樹狀） | FK thread ON DELETE CASCADE、parent ON DELETE CASCADE、owner → users |
| `thread_likes` / `comment_likes` | 按讚 | unique `(thread_id,user_id)` / `(comment_id,user_id)`；FK thread/comment ON DELETE CASCADE |
| `alembic_version` | migration 版本 | 啟動時檢查（`02-runtime.md` §2.2） |

所有業務表共有 `id varchar(256) PK`、`create_time timestamptz DEFAULT now()`、`updated_time timestamptz NULL`。

## 5.2 INSERT 時必須明確寫入的值

DB 層多數欄位沒有 default（預設值存在於 SQLAlchemy model），Rust 的 INSERT MUST 明確寫入：

| 資料表 | 欄位與值 |
| --- | --- |
| 全部 | `id` = 小寫連字號 UUID v4 字串；`create_time` 交給 DB default（或明確 `now()`）；`updated_time` = NULL |
| `users` | `is_super_user = false`；未提供的 `readable_name`/`email`/`school_department`/`note`/`hashed_password` = NULL |
| `user_preferences` | `show_empty_courses = true`、`default_is_anonymous = false`（新建使用者時）；upsert 時為請求值 |
| `users_departments` | `status = 'PENDING'`、`is_department_admin = false` |
| `posts` | `status` 依系所公開與否、`is_migrate`、`is_anonymous` 為請求值（預設 false） |
| `threads` / `thread_comments` | `like_count = 0`、`is_anonymous` 為請求值 |
| `posts_files` | `url`、`post_id` |

## 5.3 `updated_time`

參考實作以 SQLAlchemy `onupdate=func.now()` 維護（不是 DB trigger）。ORM 物件更新時，**值沒有實際改變就不會發出 UPDATE**，`updated_time` 也不變（已以參考實作驗證：對同一成員重複設定相同的管理員旗標，`updated_time` 維持原值/NULL）。Rust 版 MUST 依下表處理：

| 資料表 / 操作 | 何時設定 `updated_time = now()` |
| --- | --- |
| `users`：登入 upsert 的更新、`PUT /users/update/me` | **一律**（參考實作用 bulk `Query.update()`，即使值相同也會 UPDATE） |
| `user_preferences`：`PUT /users/me/preferences` 更新既有列 | 任一值改變時 |
| `users_departments`：核准（PENDING→APPROVED） | 一律（必定改變） |
| `users_departments`：設定管理員旗標 | 值改變時 |
| `posts`：狀態更新 | 值改變時 |
| `threads`：`PUT /threads/{id}` | 有欄位值改變時 |
| `threads` / `thread_comments`：按讚切換 | 一律（`like_count` 必定改變，P-15） |
| `thread_comments`：`PUT /threads/comments/{id}` | 內容改變時 |

`now()` 為交易開始時間（PostgreSQL 語意）；時間精度為微秒。

## 5.4 交易與併發

| 操作 | 要求 |
| --- | --- |
| 建立使用者 | `users` 與 `user_preferences` 同一交易 |
| `POST /posts` | `posts` + 所有 `posts_files` 同一交易；物件上傳失敗或 DB 失敗 → rollback，best-effort 刪除已上傳物件（L3 測試，見 `08-testing.md` §8.5） |
| `POST /threads/{course_id}` | 上傳失敗不得留下 thread 列（可先上傳再 INSERT，INSERT 失敗時刪物件；或先 INSERT 再上傳，失敗時刪列） |
| 刪除 post / thread | DB 刪除 commit 之後才刪 R2 物件；物件刪除失敗只 log |
| 按讚切換 | `SELECT … FOR UPDATE` 鎖 thread/comment 列與既有 like 列；unique 衝突 → rollback 並以 DB 現況回報 |
| 送出加入申請 | 檢查與 INSERT SHOULD 以 `pg_advisory_xact_lock(hashtext(user_id || ':' || department_id))` 序列化（現行無唯一約束） |
| `PUT /users/me/preferences` | upsert（`INSERT … ON CONFLICT (user_id) DO UPDATE`）可接受 |

## 5.5 資料完整性注意事項（現況，Rust 不修改 schema）

- `posts.course_id` 指向已不存在的課程時：`GET /posts` 會因 inner join 排除該 post，但 `GET /posts/{id}` 仍可取得。
- `posts.owner_id` 指向已不存在的使用者時：`GET /posts/{id}` 回 404，`GET /posts` 仍列出。
- 布林欄位可能為 NULL：授權判斷一律以 `IS TRUE` 處理；輸出時為 `null`。
- `varchar(256)` 欄位超長：參考實作回 500（DB 錯誤）；Rust MAY 先驗證並回 422（F-10）。

## 5.6 Schema 所有權

- **P1–P6**：Alembic（Python）仍是 schema owner；部署腳本先跑 `alembic upgrade head` / `alembic check`，Rust 只做版本檢查。
- **P7**（Python 退役）：在 `rust/migrations/` 建立 sqlx migrations：
  1. `0001_baseline.sql` = `schema.sql`（不含 `alembic_version`）。
  2. 啟動 migrator 時：若 `alembic_version.version_num = '4d7c2a91b6e3'` 且 `_sqlx_migrations` 不存在 → 將 baseline 標記為已套用（不執行），之後的 migration 照常執行。
  3. 之後的 schema 變更（例如 Q-18 的唯一約束）只透過 sqlx migration。
