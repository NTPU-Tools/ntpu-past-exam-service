# 北大考古題後端 Rust 重寫規格

本目錄是將 `ntpu-past-exam-service`（FastAPI，見 repo 根目錄）以 Rust 重寫的**規格與驗收標準**。
目標是「對前端與資料庫行為相容的替換」（drop-in replacement）：同一個前端、同一個 PostgreSQL、
同一個 Redis、同一個 R2 bucket，切換後使用者不需重新登入。

Rust 實作位於 [`rust/`](../../rust/)，已通過本規格的全部契約測試。本目錄包含：

| 路徑 | 內容 |
| --- | --- |
| `docs/rust-rewrite/01-architecture.md` | 目標/非目標、技術選型、模組切分、分階段與切換（cutover）計畫 |
| `docs/rust-rewrite/02-runtime.md` | 設定（環境變數）、啟動流程、middleware 順序、CORS、錯誤格式、輸入驗證語意、JSON 序列化規則、log |
| `docs/rust-rewrite/03-auth.md` | JWT、目前使用者解析、授權判斷式、refresh session、cookie、Origin/CSRF、rate limit、LMS 與 Google 登入 |
| `docs/rust-rewrite/04-api.md` | **每個 endpoint 的完整契約**（共 52 個 route） |
| `docs/rust-rewrite/05-data-model.md` | 資料表、欄位預設值、`updated_time` 規則、交易與併發要求 |
| `docs/rust-rewrite/06-integrations.md` | R2（S3）、SES email、Redis 快取與失效矩陣、Redis key 格式 |
| `docs/rust-rewrite/07-deviations.md` | 現行行為中的怪異點/bug：哪些**必須保留**、哪些 Rust **刻意修正**、哪些**待決定** |
| `docs/rust-rewrite/08-testing.md` | 測試策略、如何執行、**功能覆蓋矩陣**（每個功能 → 對應測試） |
| `docs/rust-rewrite/09-deployment.md` | Coolify 部署、Alembic migration 流程、啟動時 schema 相容性檢查、runbook |
| `docs/rust-rewrite/schema.sql` | 目前 PostgreSQL schema（Alembic head `4d7c2a91b6e3`） |
| `docs/rust-rewrite/assets/` | 通知信 HTML 模板（從現行程式碼原樣抽出） |
| `contract-tests/` | 黑箱 HTTP 契約測試（Rust crate），同時對現行 Python 後端與 Rust 後端執行 |
| `contract-tests/vectors/` | 無法黑箱測試之邏輯的測試向量（JSON），給 Rust 單元測試使用 |
| `contract-tests/fixtures/` | LMS HTML / Google 回應範例，給 Rust 外部整合的單元測試使用 |
| `rust/` | Rust 實作（Axum + SQLx + Redis + opendal/SES） |

## 用語

- **MUST / SHOULD / MAY**：依 RFC 2119。
- **參考實作（reference）**：目前 `main` 上的 Python 後端（commit `699d409`）。
- **契約（contract）**：前端或其他客戶端可觀察的行為：路徑、方法、狀態碼、`detail` 字串、JSON 欄位與格式、header、cookie、Redis 共用 key、R2 物件 key/URL、資料庫寫入內容。
- 規格中引用程式碼以 `檔案:行號` 表示，皆指參考實作。

## 一句話原則

> 除了 `07-deviations.md` 明列的「刻意修正」以外，Rust 版的可觀察行為 MUST 與參考實作相同；
> 有疑義時，以 `contract-tests` 對參考實作的實際執行結果為準。

## 快速開始（驗證契約測試）

```bash
# 1. 起隔離環境（Python 參考後端 + PG + Redis + MinIO；不影響本機 dev 資料）
docker compose -f contract-tests/docker-compose.yml up -d --build --wait

# 2. 對參考後端執行契約測試
cd contract-tests && cargo test

# 3. 對 Rust 後端：以 contract-tests/contract.env 啟動 rust/ 服務後
cd rust && set -a && . ../contract-tests/contract.env && set +a && cargo run -- serve
# 另一個終端：
cd contract-tests && CONTRACT_BASE_URL=http://127.0.0.1:58081 CONTRACT_TARGET=rust cargo test
```

詳見 `08-testing.md`。
