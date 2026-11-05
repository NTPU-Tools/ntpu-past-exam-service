# 06 — 外部整合：R2、Email、Redis 快取

## 6.1 R2（S3 相容）

- Client：endpoint `R2_URL`、access key `R2_ACCESS_KEY`、secret `R2_ACCESS_TOKEN`、region `auto`、path-style addressing、重試最多 10 次（standard mode 等價）。
- 上傳：`PutObject(Bucket=R2_BUCKET_NAME, Key=key, Body=bytes)`。參考實作未設定 `Content-Type`；Rust SHOULD 設定（posts → `application/pdf`，thread 圖片 → 驗證通過的宣告型別，F-08）。
- 公開 URL：`{R2_FILE_PATH}/{key}`（直接字串串接）。
- 刪除：key = URL 中第一次出現的 `{R2_FILE_PATH}/` 移除後的字串。
- Key 規則：

| 用途 | Key |
| --- | --- |
| 考古題 PDF | `{owner_user_id}/{post_id}/{uuid4}` |
| 討論串圖片 | `threads/{uuid4}` |

## 6.2 Email（AWS SES）

- 啟用條件：`AWS_EMAIL_SENDER`、`AWS_ACCESS_KEY`、`AWS_ACCESS_SECRET` 皆有值；否則略過（log `Skipping email notification: AWS SES is not configured.`）。
- Region `ap-southeast-1`（`AWS_SES_ENDPOINT` 可覆寫 endpoint 供測試）。
- `Source`：`NTPU Tools <{AWS_EMAIL_SENDER}>`。
- `Destination.ToAddresses`：收件者清單（單封信、多個收件者）；清單為空則不寄。
- 收件者位址：`users.email`，為 NULL 時 `s{username}@gm.ntpu.edu.tw`。
- 主旨（UTF-8）：`{prefix}{title} - 北大考古題 NTPU Past Exam`。`EMAIL_SUBJECT_PREFIX` 有設定時直接使用；否則 `SERVICE_NAME` 以非英數字切分後含 `prod`/`production`（不分大小寫，例如 `PROD`、`oracle-production-coolify`）→ 空字串，其餘 → `"測試環境 - "`（F-16）。
- 內文（HTML，UTF-8）：`assets/notification-email.html`，將 `{{TITLE}}`、`{{CONTENT}}` 代入，`{{CTA_BLOCK}}` 代入 `assets/notification-email-cta.html`（`{{CTA_LINK}}`、`{{CTA_TEXT}}`）或空字串（無 CTA）。兩個檔案為參考實作輸出的逐字元結果。Rust MUST 對代入值做 HTML escape（F-03）。
- 向量：`contract-tests/vectors/email.json`（E1/E2 的完整 HTML 輸出，以及 `SendEmail` 呼叫的 subject/source/to/charset；Rust 對不含特殊字元的值 MUST 產生逐字元相同的 HTML）。
- 寄信失敗 MUST NOT 影響 API 回應（只 log）；SHOULD 於 DB commit 之後以背景工作送出（F-04）。

### 事件

`{ORIGIN}` 為 `ORIGIN` 環境變數原值，`{d}` 為系所。

| 代號 | 觸發 | 收件者 | title | content | CTA（text / link） |
| --- | --- | --- | --- | --- | --- |
| E1 | 送出加入申請 | `dept_admin_recipients(d)` | `請求審核` | `有人申請瀏覽您所管理的 {d.name} 社群，請儘速審核。` | `前往審核` / `{ORIGIN}/admin/{d.id}?open_edit_member_dialog=true` |
| E2 | 核准加入 | 申請者 | `審核通過` | `您申請加入 {d.name} 的審核已通過。` | 無 |
| E3 | 設為管理員 | 該成員 | `權限變更` | `您在 {d.name} 的權限已被提升至『管理員』。` | 無 |
| E4 | 取消管理員 | 該成員 | `權限變更` | `您在 {d.name} 的權限已被變更至『一般用戶』。` | 無 |
| E5 | 私有系所新增 post | `dept_admin_recipients(d)` | `考古題審核` | `有人在您所管理的 {d.name} 社群發布考古題，請儘速審核。` | `前往審核` / `{ORIGIN}/admin/{d.id}?open_edit_post_dialog=true&edit_post_detail_id={post.id}` |
| E6 | post 狀態設為 APPROVED | post owner | `考古題審核通過` | `您在『{d.name}』發布標題為『{post.title}』的考古題已通過管理員的審核。` | `查看考古題` / `{ORIGIN}/{d.id}/{post.course_id}/{post.id}` |

E1、E5：收件者清單為空時不寄。

## 6.3 Redis 快取

### 6.3.1 快取的 endpoint

| Endpoint | namespace | TTL（秒） |
| --- | --- | --- |
| `GET /users/me/departments-admin` | `user-admin` | 30 |
| `GET /courses/{course_id}` | `course-detail` | 30 |
| `GET /departments/{department_id}/courses` | `dept-courses` | 60 |
| `GET /departments/{department_id}/bulletins` | `dept-bulletins` | 60 |
| `GET /departments/status` | `dept-status` | 60 |
| `GET /departments/visible` | `dept-visible` | 60 |
| `GET /courses` | `courses` | 60 |
| `GET /bulletins` | `bulletins` | 60 |
| `GET /bulletins/{bulletin_id}` | `bulletins` | 60 |
| `GET /posts/{post_id}` | `posts` | 60 |

其他 endpoint MUST NOT 快取（特別是 `GET /posts`、`GET /verify-token`、所有 `/threads*`、`GET /users/{id}`）。

### 6.3.2 規則

- 只快取 `200` 的 JSON body；錯誤不快取。
- 快取 key MUST 至少包含：namespace、**目前使用者 id**（參考實作用 `md5(Authorization header)`；改為 user id 讓 token 輪替後仍可命中，F-11）、route、所有 path 參數、所有 query 參數。
- 命中時：仍需通過 router-level 認證（token 有效、使用者存在），之後可直接回傳快取內容。
- Redis 錯誤 → 視同未命中並正常處理（fail-open），log warning（節流）。
- 回應 header（`ETag`、`X-FastAPI-Cache`）**不是**契約，MAY 省略；參考實作的 `If-None-Match` → 304 行為也不需要。

### 6.3.3 失效矩陣

成功回應**送出前** MUST 完成失效（下一個 request 必須看到新資料）。

| 寫入 endpoint | 失效 namespace |
| --- | --- |
| `POST /departments/{id}/join-request/send` | `dept-status` |
| `PUT /departments/join-request/{d}/approve/{r}` | `dept-status`、`dept-visible`、`posts`、`courses`、`course-detail`、`dept-courses`、`bulletins`、`dept-bulletins`、`user-admin`（F-05） |
| `PUT /departments/{id}/admin` | `user-admin`、`posts`、`courses`、`course-detail`、`dept-courses`、`bulletins`、`dept-bulletins` |
| `POST /courses/{department_id}` | `courses`、`dept-courses` |
| `POST /bulletins/{department_id}` | `bulletins`、`dept-bulletins` |
| `POST /posts` | `posts`、`course-detail`、`dept-courses` |
| `PUT /posts/status/{d}/{p}` | `posts`、`course-detail`、`dept-courses` |
| `DELETE /posts/{post_id}` | `posts`、`course-detail`、`dept-courses` |

（參考實作另外清除 `verify-token`、`user-detail`，但沒有 endpoint 使用這兩個 namespace。）

可觀察性說明：只有 `200` 會被快取，因此部分清除在黑箱下不可觀察——例如核准前申請者對私有系所的 `posts`/`course-detail`/`dept-courses`/`dept-bulletins` 請求都是 404（不會被快取）；設定管理員只會改變 `posts`（未核准 post 的可見性）與 `user-admin`。這些項目仍 SHOULD 依矩陣清除（與參考實作一致、成本低），契約測試只驗證可觀察的部分（`08-testing.md` §8.4.2）。

### 6.3.4 建議實作（非契約）

以「namespace generation counter」取代 `KEYS`/`SCAN` 刪除：

```
gen key   : {CACHE_PREFIX}:gen:{namespace}            （INCR 即失效）
entry key : {CACHE_PREFIX}:{namespace}:{gen}:{user_id}:{route}:{sha256(path+query)}  EX TTL
```

### 6.3.5 與 Python 版重疊期

`CACHE_CLEAR_LEGACY_PREFIX=true` 時，每次失效 namespace `ns` 另以 `SCAN MATCH fastapi-cache:v2:{ns}:*` + `UNLINK` 清除 Python 版快取，使同時在線的 Python instance 不會回傳舊資料。Python 版自己失效時只清自己的 prefix，因此 Rust 版在重疊期最多可能讀到 TTL（≤60 秒）內的舊資料；切換時間應盡量短。

## 6.4 Redis key 總表

| Key pattern | 擁有者 | 相容性 |
| --- | --- | --- |
| `auth:refresh-session:{sha256(sid)}` | auth | **必須與 Python 相同** |
| `auth:legacy-refresh-used:{sha256(token)}` | Python 歷史遺留 | Rust 不讀不寫（F-18） |
| `rate-limit:auth:{window}:{sha256(ident)[:32]}` | rate limit | **必須與 Python 相同** |
| `fastapi-cache:v2:{ns}:…` | Python 快取 | Rust 只刪不讀 |
| `{CACHE_PREFIX}:…` | Rust 快取 | Rust 專用 |
