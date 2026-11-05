# 01 — 架構、範圍與切換計畫

## 1.1 目標

1. 以 Rust 取代 FastAPI 後端，**前端不需任何修改**。
2. 與參考實作共用同一個 PostgreSQL schema、Redis、R2 bucket、`HASH_KEY`；切換與回滾期間：
   - Python 簽發的 access/refresh token 在 Rust 版有效，反之亦然。
   - Python 建立的 refresh session 可由 Rust 輪替（rotate）、撤銷，反之亦然。
   - 兩版可以短暫同時對外服務（blue/green 切換的重疊期）而不產生資料錯誤。
3. 修正 `07-deviations.md` 中「刻意修正（F-xx）」所列的 bug；其他行為保持一致。
4. 降低資源使用：記憶體 SHOULD < 128 MiB RSS（目前 container 上限 1536 MiB）、冷啟動 SHOULD < 1 秒。

## 1.2 非目標

- 不新增功能、不改 API 形狀、不改前端。
- 不新增系所（department）CRUD API（現行也沒有，系所由 DB/腳本維護）。
- 不移植 `scripts/seed.py`、`scripts/set_department_order.py`（維持 Python，或日後另以 CLI 子命令移植；見 §1.7）。
- 不支援 Vercel 部署（`vercel.json` 為歷史遺留，正式環境是 `deploy/docker-compose.managed.yml` 的 container）。
- 第一階段不接手 schema migration（Alembic 仍是 schema owner，見 `05-data-model.md` §5.6）。

## 1.3 技術選型（建議，非契約）

| 需求 | 建議 crate | 備註 |
| --- | --- | --- |
| HTTP server | `axum` 0.8 + `tokio` + `tower`/`tower-http` | `tower-http` 的 CORS 行為與 Starlette 不同，建議自寫 CORS layer（見 `02-runtime.md` §2.5） |
| 資料庫 | `sqlx`（postgres, runtime-tokio, chrono, macros） | 建議 compile-time checked query + `sqlx prepare` offline |
| Redis | `redis`（tokio-comp, connection-manager） | 需要 `EVAL`（Lua）、`SET NX EX`、`SCAN`/`UNLINK` 或 `INCR` |
| S3 / R2 | `aws-sdk-s3` | MUST `force_path_style(true)`、region `auto`、自訂 endpoint |
| Email | `aws-sdk-sesv2`（或 `aws-sdk-ses`） | region 固定 `ap-southeast-1` |
| JWT | `jsonwebtoken` | **leeway 必須設為 0**（預設 60 秒，與 python-jose 不同） |
| 密碼 | `bcrypt` | 驗證時與 Python bcrypt 4.x 相容（`$2b$`，72 bytes 截斷） |
| 外部 HTTP | `reqwest`（rustls） | LMS 與 Google，timeout 10 秒 |
| HTML 解析 | `scraper` | LMS 頁面解析 |
| CIDR | `ipnet` | rate limit 信任代理判斷 |
| Unicode casefold | `caseless` | rate limit subject key |
| Log | `tracing` + `tracing-subscriber`（JSON）+ 自訂 Better Stack layer | |
| multipart | `axum::extract::Multipart`（multer） | 需串流讀取並在讀取中途檢查大小 |

## 1.4 建議模組切分

```
rust/                         # 過渡期放在 repo 根目錄下；Python 退役後可上移
  Cargo.toml
  src/main.rs                 # CLI：`serve`（預設）| `healthcheck`
  src/config.rs               # 02-runtime §2.1
  src/app.rs                  # Router 組裝、layer 順序（02-runtime §2.3）
  src/error.rs                # ApiError → {"detail": ...}；422 產生器
  src/extract/                # FastAPI 相容的 Form/Query/JSON 解析（02-runtime §2.7）
  src/json_fmt.rs             # 兩種 timestamp 格式（02-runtime §2.8）
  src/auth/                   # jwt, current_user, session, cookie, origin_csrf, rate_limit, lms, google, password
  src/authz.rs                # 03-auth §3.3 授權判斷式
  src/cache.rs                # 06-integrations §6.3
  src/storage.rs              # R2
  src/mail.rs                 # SES + 模板
  src/routes/                 # health, auth, users, departments, courses, bulletins, posts, threads
  src/db/                     # 資料列型別與查詢
  tests/                      # 單元/整合測試（使用 contract-tests/vectors 與 fixtures）
```

外部依賴（LMS、Google、SES、S3、Redis、時間）MUST 以 trait 或可注入的 client 抽象，使單元測試可替換。
LMS 與 Google 的 base URL MUST 可由環境變數覆寫（見 `02-runtime.md` §2.1 的 `LMS_BASE_URL` 等），以便測試。

## 1.5 分階段計畫

| 階段 | 內容 | 驗收 |
| --- | --- | --- |
| P0 | 本規格 + 契約測試在參考實作上全綠 | `contract-tests` 對 reference 全數通過（本 branch） |
| P1 | 骨架：config、`/ping`、`/system-version`、錯誤格式、CORS、log、DB/Redis/S3 連線、JWT 驗證與目前使用者解析 | `platform`、`health`、`auth_tokens` 測試通過 |
| P2 | 唯讀 endpoint：departments / courses / bulletins / users / posts / threads 的 GET | 對應 GET 測試通過 |
| P3 | `/login`（密碼路徑）、`/refresh`、`/logout`、`/refresh/logout`、`/create_user`、rate limit；LMS / Google 以 mock 單元測試 | `auth_session`、`auth_rate_limit` 通過；`vectors/`、`fixtures/` 單元測試通過 |
| P4 | 所有寫入 endpoint、上傳、email、快取與失效 | 全部契約測試通過（`CONTRACT_TARGET=rust`，含刻意修正的測試） |
| P5 | 平行驗證：staging 以正式資料快照執行契約測試；（可選）錄製正式 GET 流量重放比對兩版回應 | 無未解釋差異 |
| P6 | 切換：沿用 `deploy/` 的 slot + Caddy 切換機制上線 Rust；回滾 = 切回 Python slot | 監控 24h 無異常 |
| P7 | Python 退役；schema 所有權移轉到 Rust（sqlx migrations，以 `schema.sql` 為 baseline） | 見 `05-data-model.md` §5.6 |

## 1.6 切換相容性要求（MUST）

1. **JWT**：同一個 `HASH_KEY`、HS256、相同 claim 結構（`03-auth.md` §3.1）。
2. **Refresh session**：Redis key 名稱、值、TTL、Lua 腳本語意完全相同（`03-auth.md` §3.4）。
3. **Rate limit**：Redis key 格式相同（`03-auth.md` §3.6），使切換時計數連續。
4. **快取**：Rust MUST 使用不同的 key prefix（避免讀到 Python 寫入的格式）；在重疊期 MUST 於失效時同時清除舊 prefix 的同名 namespace（`06-integrations.md` §6.3.5）。
5. **DB**：P1–P6 不改 schema；所有 INSERT MUST 明確填入 Python 端預設值（`05-data-model.md` §5.2），因為 DB 層多數欄位沒有 default。
6. **R2**：物件 key 與公開 URL 規則相同（`06-integrations.md` §6.1）。
7. **Cookie**：同名、同 Path、同屬性（`03-auth.md` §3.5）。

## 1.7 部署變更（交付 Rust 版時需要一併修改，位於上層 repo `deploy/`）

- `docker-compose.managed.yml` 的 backend `healthcheck` 改為 `["CMD", "/app/past-exam-service", "healthcheck"]`（Rust image 不含 Python）。
- `DATABASE_DRIVER` 對 Rust 無意義（僅支援 PostgreSQL），可保留不讀。
- Container 啟動 MUST NOT 執行 Alembic；schema 由 Python 工具 image 或部署腳本負責（P7 前）。Rust 啟動時做 schema 版本檢查（`02-runtime.md` §2.2）。
- Image：multi-stage build，runtime 用 `gcr.io/distroless/cc` 或 `debian:bookworm-slim`，非 root 使用者，`EXPOSE 8080`。
- `scripts/seed.py`、`scripts/set_department_order.py`、`deploy/migrate_mysql_to_postgres.py` 等維運腳本仍需 Python 工具 image；P7 時決定是否移植為 `past-exam-service admin ...` 子命令。
