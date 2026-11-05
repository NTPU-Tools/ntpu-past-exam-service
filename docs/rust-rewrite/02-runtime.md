# 02 — 執行期：設定、啟動、middleware、錯誤、驗證與序列化

## 2.1 環境變數

Rust 版 MUST 支援下列所有既有變數（名稱完全相同，包含拼字錯誤 `GOOGLE_SERVICE_SERCET`）。
若存在 `.env` 檔，SHOULD 以「不覆寫既有環境變數」的方式載入（等同 python-dotenv `load_dotenv()`）。
參考實作有些變數在**每個 request** 讀取（例如 `AUTH_*`），Rust 版 MAY 只在啟動時讀取一次。

### 既有變數

| 變數 | 必填 | 預設 | 用途 / 規則 |
| --- | --- | --- | --- |
| `DATABASE_HOST` / `DATABASE_PORT` / `DATABASE_USERNAME` / `DATABASE_PASSWORD` / `DATABASE` | 是 | port `5432` | PostgreSQL 連線 |
| `DATABASE_DRIVER` | 否 | — | 忽略（Rust 只支援 PostgreSQL） |
| `REDIS_HOST` / `REDIS_PORT` | 是 | — | Redis 連線 |
| `REDIS_PASSWORD` | 否 | 空 | 空字串 = 不送 AUTH |
| `R2_URL` | 是 | — | S3 相容 endpoint |
| `R2_ACCESS_KEY` / `R2_ACCESS_TOKEN` | 是 | — | access key id / secret |
| `R2_BUCKET_NAME` | 是 | — | bucket |
| `R2_FILE_PATH` | 是 | — | 公開 URL 前綴；物件 URL = `{R2_FILE_PATH}/{key}` |
| `HASH_KEY` | 是 | — | JWT HS256 密鑰；未設或空字串 MUST 啟動失敗 |
| `ORIGIN` | 建議 | — | 前端 origin：email 連結前綴、加入允許的 auth origin 與 CORS origin |
| `AUTH_ALLOWED_ORIGINS` | 否 | 空 | 逗號分隔；見 `03-auth.md` §3.5.3 |
| `AUTH_COOKIE_SECURE` | 否 | `true` | refresh cookie 是否帶 `Secure` |
| `AUTH_ALLOW_LEGACY_REFRESH_BEARER` | 否 | 未設 | 見 `03-auth.md` §3.4.4 |
| `AUTH_RATE_LIMIT_REQUESTS` | 否 | `10` | 每帳號（subject）每視窗上限 |
| `AUTH_RATE_LIMIT_GLOBAL_REQUESTS` | 否 | `300` | 每 IP 每視窗上限 |
| `AUTH_RATE_LIMIT_WINDOW_SECONDS` | 否 | `60` | 固定視窗秒數 |
| `AUTH_RATE_LIMIT_TRUST_PROXY_HEADERS` | 否 | `false` | 是否使用 `X-Forwarded-For` |
| `AUTH_RATE_LIMIT_TRUSTED_PROXY_CIDRS` | 否 | 空 | 逗號分隔 CIDR；無效項目忽略並 log warning |
| `GOOGLE_SERVICE_CLIENT_ID` / `GOOGLE_SERVICE_SERCET` | 是（登入用） | — | Google OAuth code exchange |
| `AWS_EMAIL_SENDER` / `AWS_ACCESS_KEY` / `AWS_ACCESS_SECRET` | 否 | — | 三者皆有才啟用 email；否則略過寄信（log 一行） |
| `LOG_TAIL_SOURCE_KEY` | 否 | — | 有值則送 log 到 Better Stack |
| `SERVICE_NAME` | 否 | `local` | log 的 `env` 欄位；email 主旨前綴判斷（`06-integrations.md` §6.2） |
| `COMMIT_SHA` | 否 | — | `/system-version` 回傳；未設回 `null` |

布林旗標解析：`strip().lower()` 屬於 `{"1","true","yes","on"}` 為真，其餘為假。**例外**：`AUTH_RATE_LIMIT_TRUST_PROXY_HEADERS` 只做 `lower()`、不 strip（`" true "` 為假；`auth/rate_limit.py:32`），Rust MUST 相同（向量：`vectors/client_ip.json`）。
正整數旗標解析（Python `int()` 語意，容許前後空白）：無法解析或 `<= 0` 時使用預設值。向量：`contract-tests/vectors/parsing.json`。

### Rust 新增變數（皆為選填）

| 變數 | 預設 | 用途 |
| --- | --- | --- |
| `PORT` / `BIND_ADDRESS` | `8080` / `0.0.0.0` | 監聽位址 |
| `DATABASE_URL` | — | 若設定則覆寫 `DATABASE_*` 組合 |
| `DATABASE_MAX_CONNECTIONS` | `10` | 連線池上限（參考實作 SQLAlchemy 預設 5 + overflow 10） |
| `LMS_BASE_URL` | `https://cof.ntpu.edu.tw` | 供測試覆寫 |
| `GOOGLE_TOKEN_URL` | `https://oauth2.googleapis.com/token` | 供測試覆寫 |
| `GOOGLE_USERINFO_URL` | `https://www.googleapis.com/oauth2/v1/userinfo` | 供測試覆寫 |
| `AWS_SES_ENDPOINT` | — | 供測試覆寫 SES endpoint |
| `LOG_TAIL_HOST` | `https://in.logs.betterstack.com` | Better Stack ingest host |
| `SCHEMA_CHECK` | `strict` | `strict` / `warn` / `off`，見 §2.2 |
| `CACHE_PREFIX` | `pe-cache:v3` | 快取 key prefix，MUST 不同於 `fastapi-cache:v2` |
| `CACHE_CLEAR_LEGACY_PREFIX` | `true` | 失效時是否一併清除 `fastapi-cache:v2:{ns}:*` |
| `MAX_REQUEST_BODY_BYTES` | `62914560`（60 MiB） | 整體 request body 上限（F-07） |

## 2.2 啟動流程

1. 載入設定；`HASH_KEY` 缺失 → 以非零碼結束。
2. 建立 PostgreSQL 連線池、Redis client、S3 client、（可選）SES client。Redis 可延遲連線；Redis 不可用時服務仍需啟動（各功能的失敗行為見 `03-auth.md`、`06-integrations.md`）。
3. Schema 檢查（`SCHEMA_CHECK`）：
   - `strict`：查詢 `SELECT version_num FROM alembic_version`，MUST 等於 `4d7c2a91b6e3`（或規格日後更新的 head），否則 log error 並以非零碼結束。DB 無法連線時最多重試 30 秒。
   - `warn`：不一致只 log warning。`off`：略過。
   - 參考實作的 production 啟動腳本會先跑 `alembic upgrade head && alembic check`；Rust 版 MUST NOT 自行 migrate。
4. 監聽 `BIND_ADDRESS:PORT`。
5. 收到 SIGTERM/SIGINT 時 SHOULD 停止接受新連線並等待進行中的 request（最多 10 秒）。

CLI：`past-exam-service healthcheck` MUST 對 `http://127.0.0.1:${PORT}/ping` 發 GET（timeout 3 秒），200 則 exit 0，否則 exit 1（取代 Dockerfile 的 python healthcheck）。

## 2.3 Middleware 順序（外 → 內）

1. **錯誤兜底**：handler 未處理的錯誤（DB、S3、panic…）→ `500`，`Content-Type: text/plain; charset=utf-8`，body `Internal Server Error`，並 log 錯誤與堆疊。此回應**不帶 CORS header**（與 Starlette `ServerErrorMiddleware` 位於最外層一致）。
2. **CORS**（§2.5）。preflight 在此層直接回應，不進入內層（不寫 request log、不加 Cache-Control）。
3. **Cache-Control**：`GET` 且狀態碼 2xx 的回應 MUST 設定（覆寫）`Cache-Control: no-store`。
4. **Request log**（§2.9）。
5. Router 與 handler；`HTTPException` 類錯誤轉為 JSON（§2.6）。

## 2.4 路由

- 完整 route 清單見 `04-api.md`。路由比對為大小寫敏感、完整路徑比對。
- 靜態段優先於參數段（例如 `/departments/status` 優先於 `/departments/{department_id}`；`/threads/comments/{id}` 優先於 `/threads/{thread_id}/comments`）。參考實作依註冊順序比對，兩者在 UUID id 下結果相同。
- 實作提示：`matchit`（axum）在同一位置不允許不同參數名稱時，請統一命名（例如 `/threads/{id}`、`/threads/{id}/like`），在 handler 內依語意使用。
- 未知路徑 → `404 {"detail":"Not Found"}`。路徑存在但方法不符 → `405 {"detail":"Method Not Allowed"}` 並帶 `Allow` header。
- 路徑以 `/` 結尾且去掉後能匹配 route 時，參考實作回 `307` 轉址到去掉斜線的 URL。Rust 版 SHOULD 同樣處理（非契約，前端未使用）。
- `HEAD` 不需支援（參考實作回 405）。
- 參考實作關閉了 `/docs`、`/redoc`，但 FastAPI 預設的 **`GET /openapi.json` 仍回 200**（完整 API schema，不在 52 個業務 route 內）。Rust 版不提供，回 404（F-14）。

## 2.5 CORS

允許的 origin 集合 = 下列固定清單 ∪ `03-auth.md` §3.5.3 的 auth origin 集合：

```
https://past-exam.zeabur.app
https://past-exam.ntpu.cc
https://past-exam.ntpu.xyz
http://localhost
http://localhost:5173
http://localhost:8080
```

比對方式為 request `Origin` header 與集合元素**字串完全相等**。

**Preflight**（`OPTIONS` 且同時帶 `Origin` 與 `Access-Control-Request-Method`）：

| 情況 | 狀態 | Body | Header |
| --- | --- | --- | --- |
| origin 允許 | `200` | `OK`（text/plain） | `Access-Control-Allow-Origin: <origin>`、`Access-Control-Allow-Credentials: true`、`Access-Control-Allow-Methods: DELETE, GET, HEAD, OPTIONS, PATCH, POST, PUT`、`Access-Control-Allow-Headers: <原樣回傳 Access-Control-Request-Headers>`（有帶才回）、`Access-Control-Max-Age: 600`、`Vary: Origin` |
| origin 不允許 | `400` | `Disallowed CORS origin`（text/plain） | MUST NOT 帶 `Access-Control-Allow-Origin`；其餘同上 MAY |

**一般 request** 帶 `Origin`：允許 → 回應加上 `Access-Control-Allow-Origin: <origin>`、`Access-Control-Allow-Credentials: true`、`Vary: Origin`；不允許 → MUST NOT 加 `Access-Control-Allow-Origin`（request 照常處理）。

沒有 `Access-Control-Request-Method` 的 `OPTIONS` 不是 preflight，照一般路由處理（通常 405）。

## 2.6 錯誤格式

| 種類 | 狀態 | Body |
| --- | --- | --- |
| 業務錯誤（HTTPException） | 依 endpoint | `{"detail": "<字串>"}`，`Content-Type: application/json` |
| 只指定狀態碼的錯誤 | 403 / 404 | `{"detail":"Forbidden"}` / `{"detail":"Not Found"}` |
| 驗證錯誤 | `422` | `{"detail":[{"type":…,"loc":[…],"msg":…,"input":…}, …]}` |
| 未預期錯誤 | `500` | text/plain `Internal Server Error` |

- 錯誤可帶 header（例如 429 的 `Retry-After`）。
- 每個 endpoint 的 `detail` 字串見 `04-api.md`，大小寫與標點 MUST 完全一致（前端與測試會比對）。
- 422 的每個元素 MUST 有 `type`（字串）、`loc`（陣列，如 `["body","title"]`、`["query","skip"]`）、`msg`（字串）。`input`、`ctx` SHOULD 提供。`type` 名稱 SHOULD 使用 pydantic v2 名稱：`missing`、`string_too_short`、`string_too_long`、`int_parsing`、`bool_parsing`、`greater_than_equal`、`less_than_equal`、`json_invalid`、`model_attributes_type`。
- 422 MUST 列出所有不合法欄位（不是只列第一個）。

## 2.7 輸入解析與驗證語意（FastAPI 相容）

### 2.7.1 Form

- 所有 form endpoint MUST 同時接受 `application/x-www-form-urlencoded` 與 `multipart/form-data`。其他 content type 視為空 form（→ 必填欄位 `missing`）。
- 欄位**不存在** → 必填則 `422 missing`；選填則使用預設值。
- 欄位**存在但為空字串** → 值就是 `""`，照常套用長度限制（例如 `min_length=1` → `422 string_too_short`）。**不可**視為不存在。（已以參考實作驗證；例外見 F-01。）
- 同名欄位出現多次：純量欄位取**最後一個**；檔案清單欄位（`files`）取全部。
- 字串長度以 Unicode code point 計算，不是 byte。
- 布林欄位（form 與 query）：不分大小寫、**不去除空白**，屬於 `{"1","on","t","true","y","yes"}` 為真、`{"0","off","f","false","n","no"}` 為假，其他（含 `""`、`" true"`）→ `422 bool_parsing`。向量：`vectors/parsing.json` 的 `form_and_query_bool`。
- multipart 中非檔案欄位 SHOULD 限制每個 1 MiB（超過回 `400`）；欄位與檔案數 SHOULD 各限制 1000。

### 2.7.2 Query

- 整數（pydantic v2 lax）：MUST 接受 `[+-]?[0-9]+`；SHOULD 同時接受數字間的單一底線（`1_000`）與小數部分全為 0 的寫法（`1.0`）；**不**接受前後空白、`1.5`、`1e3`、空字串 → `422 int_parsing`。超出 i64 的極大值（Python 視為合法大整數）SHOULD 視為飽和值後再套用 clamp 規則。完整向量：`contract-tests/vectors/parsing.json` 的 `query_int`。
- 未知 query 參數忽略。

### 2.7.3 JSON body（僅 `PUT /users/me/preferences`）

- body 不是合法 JSON → `422`，`loc` 以 `"body"` 開頭，`type` `json_invalid`。
- 欄位缺少 → `422 missing`；布林欄位 MUST 接受 JSON `true/false`，SHOULD 同時接受 pydantic lax 形式（`0/1`、§2.7.1 的字串）。

### 2.7.4 分頁規則

| Endpoint | `skip` | `limit` |
| --- | --- | --- |
| `GET /users` | 嚴格：`0 ≤ skip ≤ 10000`，否則 422（`greater_than_equal`/`less_than_equal`） | 嚴格：`1 ≤ limit ≤ 100`，預設 100 |
| `GET /courses`、`GET /bulletins`、`GET /posts`、`GET /threads/{course_id}`、`GET /threads/{thread_id}/comments` | clamp 到 `[0, 10000]`，預設 0 | clamp 到 `[1, 100]`，預設 100 |

### 2.7.5 檢查先後順序

| Endpoint 群組 | 順序 |
| --- | --- |
| `/users*`、`/departments*`、`/courses*`、`/posts*`、`/bulletins*` | 認證（401）→ route 授權 admin/super（401/403）→ 輸入驗證（422）→ handler |
| `POST /login`、`POST /exchange` | Origin（403）→ rate limit（429）→ 輸入驗證（422）→ handler |
| `POST /create_user` | super user 檢查（401/403）→ 輸入驗證（422）→ handler |
| `/threads*`、`GET /verify-token`、`POST /refresh`、`POST /refresh/logout`、`POST /logout` | 輸入驗證（422）→ handler（認證在 handler 內） |

（例外：JSON 語法錯誤在參考實作中會早於認證回 422；此細節非契約。）

## 2.8 JSON 序列化

- `Content-Type: application/json`；非 ASCII 字元 MUST 以 UTF-8 原樣輸出（不轉成 `\uXXXX`）。
- 物件 key 順序不重要；但**欄位集合 MUST 與 `04-api.md` 完全一致**（不可多也不可少；特別是 `hashed_password` 永遠不可出現）。
- `NULL` 欄位輸出 `null`（包含 DB 中為 NULL 的布林欄位）。
- **時間格式**（一律轉為 UTC；已以參考實作驗證）：

| 名稱 | 使用範圍 | 格式 | 範例 |
| --- | --- | --- | --- |
| `TS_OFFSET` | `/threads*` 以外的所有回應 | Python `datetime.isoformat()` | `2026-01-02T03:04:05+00:00`、`2026-01-02T03:04:05.123456+00:00` |
| `TS_Z` | 所有 `/threads*` 回應 | pydantic v2 | `2026-01-02T03:04:05Z`、`2026-01-02T03:04:05.120000Z` |

  兩者規則：微秒為 0 時不輸出小數；否則**固定 6 位**小數。
- `GET /ping` 回傳 JSON 字串 `"pong"`（不是 text/plain）。

## 2.9 Log

參考實作（`utils/log.py`、`utils/exception_handlers.py`）：

- 有 `LOG_TAIL_SOURCE_KEY` 時**只**送 Better Stack（Logtail handler）；否則只寫 **stderr**（`logging.StreamHandler()`）。
- 每個**正常完成**的 request（handler 回傳回應後，含 4xx）log 一筆 INFO：`"{METHOD} {PATH}" {STATUS} {REASON_PHRASE} {ELAPSED:.2f}ms`，附 `env = SERVICE_NAME`。未預期例外（500）時這筆 INFO 不會產生，改由例外 handler log ERROR（附堆疊）。preflight 不經過此 middleware。
- 422 時 log `{"method","path","errors":[{"type","loc"}]}`，其 `env` 欄位誤用 `R2_BUCKET_NAME`。

Rust 版要求（log 不是對外契約，以下為改進後的 MUST）：

- **MUST NOT** log query string、request body、cookie、`Authorization`、token、密碼、OAuth code。
- 422 log 只含 `method`、`path`、`errors[].type`、`errors[].loc`，MUST NOT 含 `input`、`ctx`、`msg`（`msg` 可能夾帶輸入值）。
- 每個 request（含 500，preflight 可略）log 一筆 INFO，格式同上；`env` 一律使用 `SERVICE_NAME`（F-15）。
- 未預期錯誤 log ERROR 並附錯誤鏈。
- Sink：一律輸出到 stdout（JSON lines SHOULD，方便 container log）；若設定 `LOG_TAIL_SOURCE_KEY`，**另外**以 HTTP 批次送到 Better Stack（`POST {LOG_TAIL_HOST}`，`Authorization: Bearer <key>`，JSON 陣列，每筆至少 `dt`、`level`、`message`、`env`）。送 log 失敗 MUST NOT 影響 request。
- 外部依賴暫時失敗（rate limiter 用的 Redis）以「每分鐘最多一次」節流 log（見 `03-auth.md` §3.6）。
