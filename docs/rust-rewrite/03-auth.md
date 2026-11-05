# 03 — 認證、授權與 Session

參考實作：`auth/*.py`、`utils/token.py`、`users/dependencies.py`、`departments/dependencies.py`。

## 3.1 JWT

### 3.1.1 簽發

- 演算法 HS256，金鑰為 `HASH_KEY` 的 UTF-8 bytes。Header `{"alg":"HS256","typ":"JWT"}`。
- 不含 `iat`、`nbf`、`aud`、`iss`。`exp` 為整數秒（Unix time）。

**Access token**（壽命 900 秒 = 15 分鐘）：

| claim | 值 |
| --- | --- |
| `sub` | `users.id` |
| `id` | `users.id` |
| `type` | `"access"` |
| `isu` | `users.is_super_user`（DB 為 NULL 時輸出 `null`） |
| `adm` | **字串**：`admin_dept_ids(user)`（§3.3）以 Python `json.dumps` 格式序列化，例如 `"[]"`、`"[\"d1\", \"d2\"]"`（元素間分隔為 `", "`） |
| `exp` | now + 900 |

**Refresh token**（壽命 365 天 = 31 536 000 秒）：同上，但 `type` 為 `"refresh"`，另加：

| claim | 值 |
| --- | --- |
| `session_version` | 整數 `1` |
| `sid` | session family id（UUID v4 字串；輪替時沿用） |
| `jti` | 本 token id（UUID v4 字串；每次簽發都換新） |
| `exp` | now + 31 536 000 |

`isu`/`adm` 只是資訊性 claim，**MUST NOT** 用於授權判斷（必須查 live DB，見 §3.3）。

向量：`contract-tests/vectors/jwt.json`（Python 簽發的 access/refresh token 範例與 `adm` 序列化格式；Rust MUST 能驗證這些 token）。

### 3.1.2 驗證 `decode(token, expected_type)`

依序檢查，任一失敗 → `401 {"detail":"Invalid or expired token"}`：

1. 結構為三段 JWS，簽章以 HS256 + `HASH_KEY` 驗證通過；其他演算法（含 `none`）一律拒絕。
2. `exp` 必須存在且為數字；`now > exp` 視為過期。**leeway = 0**。
3. `sub` 必須存在且為字串。
4. 若有 `nbf`：`now < nbf` 拒絕。若有 `iat`：必須為數字。若有 `aud`：拒絕（未設定預期 audience）。若有 `jti`：必須為字串。

接著：`payload.type != expected_type`（含缺少 `type`）→ `401 {"detail":"Invalid token type"}`。

### 3.1.3 從 `Authorization` header 取 access token

header 值以**單一空白**切割，必須恰好兩段且第一段為大小寫敏感的 `Bearer`，否則
`401 {"detail":"Invalid or missing authorization header"}`。（`Bearer `（空 token）會通過此步驟並在 decode 失敗；`Bearer  x`（兩個空白）在此步驟失敗。）
之後 `decode(token, "access")`。

## 3.2 目前使用者解析

### 模式 A：完整解析（router-level 認證、admin/super guard、`/verify-token`、`/refresh`）

1. `payload.id` 為真值 → 以 `users.id = payload.id` 查詢。
2. 若沒找到且 `payload.sub` 為真值 → 依序以 `users.id = sub`、`users.username = sub` 查詢（相容早期 token）。
3. 仍沒有 → `401 {"detail":"Invalid credentials"}`。

### 模式 B：thread 端點

1. `payload.id` 為假值 → `401 {"detail":"Invalid token: missing user id"}`。
2. `users.id = payload.id` 查無 → `401 {"detail":"User not found"}`。

### Handler 內的「viewer id」

參考實作的 handler 以 `payload.id` 作為目前使用者 id（模式 A 只負責把關）。由系統簽發的 token 一定有 `id`，因此 Rust 版 MAY 一律使用模式 A 解析出的使用者 id（F-09）。
`POST /posts`、`DELETE /posts/{post_id}` 另外在 `payload.id` 為假值時回 `401 {"detail":"Invalid token: missing user id"}`。

## 3.3 授權判斷式（全部查 live DB）

設 `u` 為使用者、`d` 為系所、`m(u,d)` 為 `users_departments` 中 `(user_id=u, department_id=d)` 的列（理論上至多一筆；若有多筆，任一符合即可）。

| 名稱 | 定義 |
| --- | --- |
| `is_super(u)` | `u.is_super_user IS TRUE` |
| `approved(u,d)` | ∃ `m(u,d)`：`status = 'APPROVED'` |
| `dept_admin(u,d)` | ∃ `m(u,d)`：`status = 'APPROVED'` ∧ `is_department_admin IS TRUE` |
| `admin_dept_ids(u)` | `is_super(u)` ? 全部系所 id : `{d | dept_admin(u,d)}` |
| `can_view_dept(u,d)` | `is_super(u)` ∨ `d.is_public IS TRUE` ∨ `approved(u,d)` |
| `visible_dept_ids(u)` | `is_super(u)` ? 全部系所 : `{d | d.is_public IS TRUE ∨ approved(u,d)}`；依 `sort_order ASC` 排序 |
| `can_view_unapproved_post(u,p)` | `is_super(u)` ∨ `p.owner_id = u.id` ∨ `dept_admin(u, p.department_id)` |
| `can_view_user_details(v,t)` | `v = t` ∨ `is_super(v)` ∨ ∃ `d`：`dept_admin(v,d)` ∧ ∃ `m(t,d)`（**任何 status**，讓管理員能看 pending 申請者的 note） |
| `dept_admin_recipients(d)` | 所有 `m(·,d).is_department_admin IS TRUE` 的使用者（**不檢查 status**），用於 email 通知 |

### 共用檢查

- **`require_viewable_department(u, d_id)`**：`u` 不存在 → `401 "User not found"`；系所不存在 → `404 "Department not found"`；`!can_view_dept` → `404 "Department not found"`（不揭露私有系所存在）。
- **Route guard `admin`**（路徑含 `{department_id}`）：模式 A 解析 → `is_super` 直接通過 → 否則 `department_id ∈ admin_dept_ids(u)`，不符 → `403 {"detail":"Forbidden"}`。不檢查系所是否存在。
- **Route guard `super`**：模式 A 解析 → `!is_super` → `403 {"detail":"Forbidden"}`。

## 3.4 Refresh session（Redis）

### 3.4.1 常數與 key

| 名稱 | 值 |
| --- | --- |
| Session key | `auth:refresh-session:{sha256_hex(sid)}`，值 = 目前有效的 `jti`，TTL = 31 536 000 |
| Legacy 已用 key | `auth:legacy-refresh-used:{sha256_hex(完整 token 字串)}`，值 = `"1"`，TTL = 該 legacy token 剩餘秒數 |
| `REFRESH_SESSION_VERSION` | `1` |
| Legacy 遷移截止 | `2026-11-03T00:00:00Z` |

### 3.4.2 判斷 token 種類

- **managed**：`session_version == 1` 且 `sid`、`jti` 皆為非空字串。
- **legacy**：`sid`、`jti` 皆為假值（不存在、`null`、`""`）。
- 其餘（例如只有 `jti`）為 **unknown**。

### 3.4.3 操作（Lua 腳本 MUST 與下列語意等價且具原子性）

**註冊（登入時）**：`SET session_key jti EX 31536000 NX`。Redis 錯誤或不可用 → `503 "Refresh session store is unavailable"`；NX 失敗 → `503 "Could not create refresh session"`。

**輪替**：
```lua
-- refresh-session-rotate
local current_jti = redis.call('GET', KEYS[1])
if not current_jti or current_jti ~= ARGV[1] then
  return 0
end
redis.call('SET', KEYS[1], ARGV[2], 'EX', ARGV[3])
return 1
```
回傳 0 → `401 "Refresh session is no longer active"`；Redis 錯誤 → `503 "Refresh session store is unavailable"`。

**Legacy 遷移**：
```lua
-- refresh-session-migrate-legacy
if redis.call('EXISTS', KEYS[1]) == 1 then
  return 0
end
redis.call('SET', KEYS[1], '1', 'EX', ARGV[1])
redis.call('SET', KEYS[2], ARGV[2], 'EX', ARGV[3])
return 1
```
KEYS = [legacy 已用 key, 新 session key]；ARGV = [legacy 剩餘秒數, 新 jti, 31536000]。回傳 0 → `401 "Legacy refresh token has already been used"`。
legacy 剩餘秒數 = `int(exp - now)`，`<= 0` → `401 "Invalid or expired token"`。

**撤銷**：managed → `DEL session_key`（撤銷整個 family，可防「輪替與登出競態」）；legacy → `SET legacy_key "1" EX 剩餘秒數 NX`；unknown → `401 "Unknown refresh session"`；Redis 錯誤 → `503`。

### 3.4.4 Legacy bearer 開關

`AUTH_ALLOW_LEGACY_REFRESH_BEARER` 有設定 → 依布林旗標解析；未設定 → `now < 2026-11-03T00:00:00Z`。
（目前日期已過截止日，未設定時等同關閉。是否在 Rust 版直接移除見 Q-06。）向量：`vectors/legacy_refresh_window.json`。

## 3.5 Cookie、Origin 與 CSRF

### 3.5.1 Refresh cookie

| 屬性 | 值 |
| --- | --- |
| 名稱 | `ntpu-past-exam-refresh-token-v2` |
| Path | `/refresh`（因此瀏覽器只會在 `/refresh`、`/refresh/logout` 帶上） |
| Max-Age / Expires | `31536000` / now + 365 天（HTTP-date） |
| HttpOnly | 是 |
| SameSite | `lax` |
| Secure | `AUTH_COOKIE_SECURE`（預設 true） |
| Domain | **不設定**（host-only） |

刪除 cookie：同名、同 Path、同 HttpOnly/SameSite/Secure，值為空（參考實作輸出 `""`），`Max-Age=0`，`expires` 為現在或過去。

### 3.5.2 No-store

所有 auth 成功回應（`/login`、`/exchange`、`/refresh`、`/refresh/logout`、`/logout`、`/verify-token`、`/create_user`）MUST 帶 `Cache-Control: no-store` 與 `Pragma: no-cache`。

### 3.5.3 允許的 auth origin

`AUTH_ALLOWED_ORIGINS` 與 `ORIGIN` 以逗號串接後切割，每項 `strip()` 並去掉結尾 `/`，忽略空項。
**只**使用環境變數，不包含 CORS 固定清單（§2.5）。向量：`vectors/auth_origins.json`（含 §3.5.4 的檢查結果）。

### 3.5.4 檢查

- **`validate_origin(required)`**：沒有 `Origin` header → required 時 `403 "Origin header is required"`，否則通過；`Origin` 為 `"null"` 或不在集合中 → `403 "Origin is not allowed"`。
- **`validate_browser_mutation`**：`validate_origin(required=true)`，且 `X-CSRF-Protection` header MUST 恰為 `1`，否則 `403 "X-CSRF-Protection header is required"`。

## 3.6 Auth rate limit（Redis 固定視窗）

適用：`POST /login`（global + subject）、`POST /exchange`（只有 global）。在 Origin 檢查**之後**、輸入驗證**之前**執行（被拒絕的 Origin 不消耗額度；欄位缺漏的 request 仍會消耗額度）。

### 3.6.1 Client IP

1. `direct` = TCP peer IP。
2. `AUTH_RATE_LIMIT_TRUST_PROXY_HEADERS` 為假 → 使用 `direct`。
3. `direct` 不是合法 IP，或信任 CIDR 清單為空，或 `direct` 不在信任清單 → 使用 `direct`。
4. 解析 `X-Forwarded-For`（逗號分隔、strip、忽略空項）；任一項不是合法 IP → 使用 `direct`。
5. 由**右往左**找第一個不在信任清單的位址並使用之；全部可信 → 使用 `direct`。
6. IPv4 位址只與 IPv4 網段比對、IPv6 只與 IPv6 比對（`::ffff:a.b.c.d` 不視為 IPv4）。

測試向量：`contract-tests/vectors/client_ip.json`。

### 3.6.2 Key 與計數

```
window      = floor(now / W)                    # W = AUTH_RATE_LIMIT_WINDOW_SECONDS
window_ttl  = W - (now mod W)
key(ident)  = "rate-limit:auth:{window}:{sha256_hex(ident)[0:32]}"
global      = key("global:{ip}")                       上限 AUTH_RATE_LIMIT_GLOBAL_REQUESTS
subject     = key("subject:{ip}:{casefold(subject)}")  上限 AUTH_RATE_LIMIT_REQUESTS
subject     = "login:" + strip(form.username)    # 只在 /login 且 username 非空時
```

每個 key 以下列腳本計數（`ARGV[1] = window_ttl`，subject key 也使用 global 算出的 `window_ttl`）：

```lua
local current = redis.call('INCR', KEYS[1])
if current == 1 then
  redis.call('EXPIRE', KEYS[1], ARGV[1])
end
local ttl = redis.call('TTL', KEYS[1])
return {current, ttl}
```

先檢查 global，再檢查 subject。`current > limit` → `429 {"detail":"Too many authentication attempts"}`，header `Retry-After: max(ttl, 1)`。
`casefold` 為 Unicode full case folding（Python `str.casefold()`），`strip` 去除 Unicode 空白。
`username` 取 form 中同名欄位的最後一個值。
向量：`vectors/rate_limit.json`（key、`window_ttl`、strip + casefold 結果）。

### 3.6.3 失敗策略

Redis 不可用或錯誤 → **放行**（fail-open），並以「每 60 秒最多一次」log warning `Auth rate limiter unavailable; allowing request`。
注意：放行之後的 session 註冊仍會因 Redis 不可用而 `503`（fail-closed）。

## 3.7 登入流程

### 3.7.1 `POST /login`（帳號密碼）

Form（OAuth2PasswordRequestForm）：`username`（必填）、`password`（必填）、`grant_type`（選填；若提供 MUST 符合 `^password$`，否則 422）、`scope`、`client_id`、`client_secret`（選填，忽略）。

1. `user = users.username = form.username`（原字串，精確比對）。
2. **LMS 資格**：`form.username` 能以 Python `int()` 語意解析為整數（前後空白、正負號、數字間單一底線、Unicode 十進位數字皆允許），則以 `str(int(username))`（去前導零的 ASCII 十進位）呼叫 LMS（§3.7.3）。向量：`vectors/lms_username.json`。
   - LMS 回報帳密錯誤 → `400 {"detail":"Incorrect username or password"}`（大寫 I）。**不會**退回本地密碼驗證。
   - LMS 網路錯誤/逾時/頁面結構不符 → `500`。
   - 成功 → upsert 使用者（§3.7.5），登入成功。
3. **非 LMS 資格** → 本地密碼：`user` 不存在或 `hashed_password` 為空 → `400 {"detail":"incorrect username or password"}`（小寫 i）；bcrypt 驗證失敗 → 同一個 400。
4. 簽發 session（§3.7.6）。

### 3.7.2 `POST /exchange`（Google OAuth）

1. `POST GOOGLE_TOKEN_URL`（form）：`client_id`、`client_secret`（=`GOOGLE_SERVICE_SERCET`）、`code`、`redirect_uri`、`grant_type=authorization_code`，timeout 10 秒。
2. 回應 JSON 沒有 `access_token` → `403 {"detail":"Forbidden"}`。
3. `GET GOOGLE_USERINFO_URL`，header `Authorization: Bearer {access_token}`。
4. 回應沒有 `hd` 或 `hd != "gm.ntpu.edu.tw"` → `403 {"detail":"Forbidden"}`。
5. `school_id` = `email` 的第 2～10 個字元（Python `email[1:10]`，以 code point 計，長度不足時取到結尾）。
6. Upsert（§3.7.5）後簽發 session。

缺少 `email`/`name`、回應非 JSON、網路錯誤：參考實作回 500；Rust MUST NOT 登入成功，回 403 或 500 皆可（非契約）。
Fixtures：`contract-tests/fixtures/google/`；預期結果與實際送出的 request：`vectors/google.json`、`vectors/google_school_id.json`。

### 3.7.3 LMS（`LMS_BASE_URL`，預設 `https://cof.ntpu.edu.tw`）

1. `POST {LMS}/pls/pm/stud_system.login`，form：`stud_num=<正規化學號>`、`passwd=<密碼>`、`x=109`、`y=15`；timeout 10 秒；保留回應 cookie（跟隨 redirect）。
2. 回應 HTML 中**存在任何 `<h3>`** → 帳密錯誤。
3. 取 `<body onload="...">`，擷取 `window.open('../univer/query_all_course.login2?date1=` 與 `','_top')` 之間的字串為 `date1`。
4. `GET {LMS}/pls/univer/query_all_course.judge?func=10&date1={date1}`（帶 cookie）：找到文字節點**完全等於** `(選課說明：` 的元素，取其父元素底下所有 `<span>`（後代，文件順序）：第 4 個（index 3）文字 = `readable_name`，第 2 個（index 1）= `department`。
5. `GET ...?func=18&date1={date1}`：第一個 `<input type="email">` 的 `value` 屬性 = `email`（可能不存在 → NULL）。
6. 回應編碼依 `Content-Type` charset；未指定時 SHOULD 以 UTF-8 解碼（待以真實帳號確認，Q-17）。

Fixtures：`contract-tests/fixtures/lms/*.html`（依 parser 邏輯合成，非真實頁面）；以參考實作 parser 執行的結果與實際送出的 request：`vectors/lms.json`。

### 3.7.4 密碼

- 驗證：bcrypt（`$2a$`/`$2b$`/`$2y$`），密碼取 UTF-8 bytes 並截斷至 72 bytes（與 Python bcrypt 4.x 相同）。儲存的 hash 格式錯誤：參考實作回 500，Rust MAY 回 400 `incorrect username or password`。
- 產生（`/create_user`）：bcrypt cost 12、`$2b$` 前綴。
- 向量：`vectors/bcrypt.json`。

### 3.7.5 使用者 upsert

| 來源 | 新建 | 已存在時更新 |
| --- | --- | --- |
| LMS | `username`=form 原值、`readable_name`、`school_department`=LMS department、`email`、`hashed_password`=NULL、`is_super_user`=false | `readable_name`、`school_department`、`email`（**不動** `note`，F-02；不動 `hashed_password`） |
| Google | `username`=`school_id`、`readable_name`=`name`、`school_department`=`" "`（一個空白）、`email` | `readable_name`、`email`（`school_department` 不變；**不動** `note`，F-02） |
| `/create_user` | `username`、`readable_name`=username、`school_department`=NULL、`email`=NULL、`hashed_password`=bcrypt | —（重複 → 400） |

所有新建使用者 MUST 同一交易建立 `user_preferences`（`show_empty_courses=true`、`default_is_anonymous=false`）。
所有更新 MUST 設定 `updated_time = now()`。

### 3.7.6 簽發 session

1. 計算 `admin_dept_ids(user)`，產生 access token 與 refresh token（新 `sid`、新 `jti`）。
2. 註冊 session（§3.4.3），失敗 → 503（不回 token、不設 cookie）。
3. 回 `200 {"access_token": "<jwt>", "token_type": "bearer"}`（**只有這兩個 key**；refresh token 只放 cookie），設定 refresh cookie，加 no-store header。
