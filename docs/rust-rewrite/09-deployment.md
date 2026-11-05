# 9. 部署與 Schema Migration

Alembic 是唯一的 schema owner；Rust 不執行 migration，只在啟動時檢查相容性。
（由 Coolify 4.3.23 實測：`pre_deployment_command` 是在**舊容器**內 `docker exec`，看不到新 image 的 migration，不能用來跑 Alembic。）

## 9.1 部署流程（Coolify Docker Compose build pack）

`docker-compose.coolify.yml`：

1. `migrate`：用 repo 根目錄的 Python `Dockerfile` 執行 `alembic upgrade head && alembic check`，`restart: "no"`（Coolify 會因此不把它列入健康狀態）。
2. `api`：`rust/Dockerfile`，`depends_on: migrate: condition: service_completed_successfully`。migration 失敗 → `api` 不會啟動。

push 到 upstream `main` → Coolify webhook → build 兩個 image → migrate → api。

### Coolify app 設定

| 項目 | 值 |
| --- | --- |
| Build pack | Docker Compose |
| Compose 檔 | `/docker-compose.coolify.yml` |
| Connect To Predefined Network | **關閉**。compose 檔自行加入 `coolify` 網路並設定 alias `past-exam-rust-api`；開啟此選項時 Coolify 會覆寫該網路設定、alias 會消失 |
| 環境變數 | 沿用 `past-exam-api-main`（`DATABASE_*`、`REDIS_*`、`R2_*`、`HASH_KEY`、`ORIGIN`、`AWS_*`、`GOOGLE_*`、`SERVICE_NAME`、`LOG_TAIL_SOURCE_KEY`）；`COMMIT_SHA` 由 Coolify 的 `SOURCE_COMMIT` 帶入 |
| Domain | 不設定，流量由外部 Caddy 轉發 |

Cutover：Caddy 的 `past-exam-api.ntpu.cc` upstream 改為 `past-exam-rust-api:8080`。

## 9.2 啟動時的 schema 檢查

`rust/schema_revisions.txt` 是 Alembic revision 鏈（舊 → 新，最後一行是 head），由 `scripts/sync_schema_revisions.py` 產生並編入 binary。

| DB `alembic_version` | 判定 | `SCHEMA_CHECK=strict` | `warn` |
| --- | --- | --- | --- |
| = head | Current | 啟動 | 啟動 |
| 清單內較舊的 revision | Behind | **以非零碼結束** | warning 後啟動 |
| 不在清單內 | Ahead（較新的 migration） | warning 後啟動 | warning 後啟動 |
| 空表 | Missing | 以非零碼結束 | warning 後啟動 |

DB 比 binary 新時照常啟動，這樣 migration 套用後，舊版 Rust 容器重啟或 rollback 仍能運作。前提是 §9.3 的相容性規則。

## 9.3 Migration 規則

- **Expand / contract**：新增欄位先 nullable 或有 default；rename / drop 分成多次 release（加新欄位 → 雙寫 / backfill → 移除舊欄位）。同一個 release 不得 rename 或 drop 仍在使用的欄位。
- 大表 index 用 `CREATE INDEX CONCURRENTLY`（Alembic 要放在 `autocommit_block()`）。
- Migration 開頭設 `SET lock_timeout = '5s'`，避免卡住線上查詢。
- Production 不依賴 `alembic downgrade`；改用往前修正的新 migration 加 DB 備份。
- 不修改已套用到 production 的 migration 檔。

## 9.4 新增 migration 的步驟

```bash
poetry run alembic revision -m "add xxx"      # Python models 已不維護時一律手寫
python scripts/sync_schema_revisions.py        # 更新 rust/schema_revisions.txt
cd rust && cargo test                          # schema_revisions_match_the_alembic_history 會擋住忘記同步
```

`python scripts/sync_schema_revisions.py --check` 可放進 CI；Alembic 必須維持單一線性 head（腳本遇到分支或 merge revision 會失敗）。

## 9.5 Runbook

1. 備份：`pg_dump -Fc past_exam > past_exam-$(date +%F).dump`
2. 合併到 upstream `main`，等待 Coolify deployment 完成。
3. 驗證：
   - `migrate` 容器 exit 0；`api` 容器 healthy。
   - `SELECT version_num FROM alembic_version` = `rust/schema_revisions.txt` 最後一行。
   - `api` 容器的 `COMMIT_SHA` = 剛合併的 commit。**不要只看網站回 200**：Caddy 有 fallback upstream，primary 掛掉時仍會回 200。
4. Rollback：Coolify 重新部署上一個 commit。依 §9.2，舊 binary 在較新的 schema 上可以啟動；schema 本身不回退。
