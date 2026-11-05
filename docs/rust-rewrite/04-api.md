# 04 — API 契約（52 個 route）

通用規則（每個 endpoint 皆適用，除非另註）：

- 錯誤格式、422、500：`02-runtime.md` §2.6。輸入語意與檢查順序：§2.7。時間格式：§2.8。
- 「認證 A」= `03-auth.md` §3.2 模式 A；「認證 B」= 模式 B。
- 「guard admin」/「guard super」= `03-auth.md` §3.3 的 route guard。
- `require_viewable_department` 與各判斷式見 `03-auth.md` §3.3。
- 「快取」欄位：namespace / TTL 秒；「失效」欄位：成功後必須失效的 namespace（`06-integrations.md` §6.3）。
- 所有 GET 2xx 回應帶 `Cache-Control: no-store`（§2.3）。
- 401 的共通 detail：`Invalid or missing authorization header`、`Invalid or expired token`、`Invalid token type`、`Invalid credentials`（模式 A）、`Invalid token: missing user id` / `User not found`（模式 B）。以下各 endpoint 不再重複列出。

## 4.0 共用回應物件

`*` 表示套用匿名遮罩：`is_anonymous IS TRUE` 時 `owner_id = "anonymous"`、`owner_name = "匿名用戶"`；否則 `owner_name = readable_name`（非空時）否則 `username`。

| 物件 | 欄位（**恰好**這些 key） | 時間格式 |
| --- | --- | --- |
| `Department` | `id, create_time, updated_time, key, name, is_public, sort_order` | TS_OFFSET |
| `Course` | `id, create_time, updated_time, name, category, department_id` | TS_OFFSET |
| `CourseBrief` | `id, name, category` | — |
| `DeptCourse` | `Course` + `has_posts`（該課程存在 `status='APPROVED'` 的 post） | TS_OFFSET |
| `Bulletin` | `id, create_time, updated_time, title, content, department_id` | TS_OFFSET |
| `User` | `id, create_time, updated_time, email, username, readable_name, note, school_department, is_super_user` | TS_OFFSET |
| `UserMe` | `User` + `show_empty_courses, default_is_anonymous`（無 preference 列時 `true`/`false`） | TS_OFFSET |
| `UserRedacted` | `id, create_time, updated_time, username, readable_name, note, school_department` | TS_OFFSET |
| `Membership` | `id, create_time, updated_time, user_id, department_id, status, is_department_admin` | TS_OFFSET |
| `JoinRequest` | `id`（membership id）`, status`（固定 `"PENDING"`）`, user_id, email, username, readable_name, school_department, note` | — |
| `Member` | `id`（= user id）`, user_id, email, username, readable_name, school_department, is_department_admin` | — |
| `PostRow` | `id, create_time, updated_time, title, content, owner_id*, course_id, is_migrate, is_anonymous, status, department_id` | TS_OFFSET |
| `PostListItem` | `PostRow` + `course_name` | TS_OFFSET |
| `PostDetail` | `PostRow` + `files`（`posts_files.url` 字串陣列）+ `owner_name*` | TS_OFFSET |
| `CoursePost` | `PostRow` 去掉 `status` 與 `owner_id`（不是遮罩，是整個移除） | TS_OFFSET |
| `Thread` | `id, title, content, image_url, course_id, is_anonymous, like_count, create_time, updated_time, owner_id*, owner_name*, is_owner, liked` | TS_Z |
| `Comment` | `id, thread_id, parent_comment_id, content, is_anonymous, like_count, create_time, updated_time, owner_id*, owner_name*, is_owner, liked, reply_count, replies` | TS_Z |

- `Thread.like_count`、`Comment.like_count`：DB 為 NULL 時輸出 `0`。`is_owner` = 目前使用者 id 等於 `owner_id`（遮罩前）。
- `Comment.reply_count`、`Comment.replies`、`Thread.liked` 在某些 endpoint 為 `null`，見各 endpoint。
- 排序未定義處，Rust 版 SHOULD 使用 `(create_time ASC, id ASC)` 等穩定排序（F-06）；測試不依賴未定義的順序。

---

## 4.1 Health

### `GET /ping`
無認證。`200`，body 為 JSON 字串 `"pong"`。

### `GET /system-version`
無認證。`200 {"GIT_SHA": <COMMIT_SHA 或 null>}`。

---

## 4.2 Auth（無 router-level 認證）

### `POST /login`
- 前置：`validate_origin(required=false)` → rate limit（global + subject）→ 驗證 form。
- 流程：`03-auth.md` §3.7.1、§3.7.6。
- 成功：`200 {"access_token","token_type":"bearer"}` + refresh cookie + no-store。
- 錯誤：`403 Origin is not allowed`；`429 Too many authentication attempts`（`Retry-After`）；`422`；`400 incorrect username or password`；`400 Incorrect username or password`（LMS）；`503 Refresh session store is unavailable` / `Could not create refresh session`。

### `POST /exchange`
- Form：`code`（必填）、`redirect_uri`（必填）。
- 前置：`validate_origin(required=false)` → rate limit（只有 global）→ 驗證 form。
- 流程：`03-auth.md` §3.7.2、§3.7.6。回應同 `/login`。
- 錯誤：`403 Origin is not allowed`；`429`；`422`；`403 Forbidden`（Google 拒絕或非 `gm.ntpu.edu.tw`）；`503`。

### `GET /verify-token`
- 認證 A（handler 內）。
- `200 {"admin": admin_dept_ids(u), "visible_departments": visible_dept_ids(u)}`（id 字串陣列；`visible_departments` 依 `sort_order` 排序，`admin` 順序未定義）+ no-store。
- 前端 `proxy.ts` / `route-support.tsx` 以 `visible_departments` 判斷能否進入系所頁。

### `POST /refresh`
1. 若 request 帶非空的 refresh cookie：`validate_browser_mutation`（403），token = cookie。否則 token = `Authorization: Bearer <非空>`，格式不符 → `401 Invalid credentials`。（cookie 優先：同時帶 cookie 與 bearer 時走 cookie 規則。）
2. `decode(token, "refresh")`。
3. 種類（§3.4.2）：unknown → `401 Unknown refresh session`；legacy 且（來自 cookie 或 legacy 開關關閉）→ `401 Legacy refresh bearer migration is disabled`。
4. 模式 A 解析使用者（`401 Invalid credentials`）。此步驟在輪替**之前**，使用者已刪除時不會消耗 session。
5. 以 live DB 重新產生 access/refresh token（managed 沿用 `sid`，legacy 產生新 `sid`），新 `jti`。
6. managed → 輪替；legacy → 遷移（§3.4.3）。
7. `200 {"access_token","token_type":"bearer"}` + 新 refresh cookie + no-store。
- 錯誤另含：`401 Refresh session is no longer active`、`401 Legacy refresh token has already been used`、`503 Refresh session store is unavailable`。

### `POST /refresh/logout`
1. `validate_browser_mutation`（不論是否帶 cookie）。
2. 若有 refresh cookie：`decode(token,"refresh")`；decode 失敗則忽略（不撤銷）；成功則撤銷（§3.4.3；unknown 種類 → `401 Unknown refresh session`；Redis 錯誤 → 503）。
3. 刪除 refresh cookie，no-store，`200 {"status":"success"}`。

### `POST /logout`
相容舊客戶端。`validate_browser_mutation` → 刪除 refresh cookie（瀏覽器不會把 Path=/refresh 的 cookie 送到這裡，因此不撤銷）→ no-store → `200 {"status":"success"}`。

### `POST /create_user`
- guard super（認證 A）→ form `username`、`password`（必填；OAuth2 form 其他欄位同 `/login`）。
- `username` 或 `password` 為假值 → `400 Invalid input`（實務上空字串仍是假值，故 `username=""` → 400）。
- 已有同 username → `400 Same username already exist.`。
- 建立使用者（`03-auth.md` §3.7.5）。
- `200 {"status":"success","user_id": "<id>"}` + no-store；**不**設 cookie、**不**回 token（不得替換呼叫者的 session）。

---

## 4.3 Users（router-level：認證 A）

### `GET /users`
- guard super。Query：`is_active`（布林，選填）、`skip`（0..=10000，預設 0）、`limit`（1..=100，預設 100），超出 → 422。
- `is_active` 有提供（合法布林）→ `410 The is_active user field is no longer supported`。
- `200 [User, …]`（offset/limit，順序未定義）。

### `GET /users/{user_id}`
- `user_id == "me"` → 目標 = 自己，`is_me = true`。
- `!can_view_user_details(viewer, target)` → `403 Not allowed to view this user`（先於存在性檢查：一般使用者查不存在的 id 得到 403；super 得到 404）。
- 目標不存在 → `404 User not found`。
- `is_me` → `200 UserMe`；否則（**包含以自己的 id 查詢**）→ `200 UserRedacted`。

### `GET /users/me/departments-admin`
- `200 [Department, …]`：`is_super` → 全部系所；否則 `dept_admin(u,d)` 的系所。順序未定義。
- 快取：`user-admin` / 30。

### `PUT /users/update/me`
- Form：`major`（必填，可為空字串）、`note`（選填，預設 `""`）。
- 更新 `school_department = major`、`note = note`、`updated_time = now()`；其他欄位不變。
- `200 UserMe`（更新後）。
- 失效：`user-detail`、`verify-token`（參考實作中這兩個 namespace 沒有任何快取 endpoint 使用；Rust MAY 省略）。

### `PUT /users/me/preferences`
- JSON：`{"show_empty_courses": bool, "default_is_anonymous": bool}`（皆必填）。
- Upsert `user_preferences`（`user_id` 唯一）。
- `200 {"show_empty_courses": bool, "default_is_anonymous": bool}`。
- 失效：`user-detail`、`verify-token`（同上，沒有 endpoint 使用；Rust MAY 省略）。

### `PUT /users/status/{department_id}/{user_id}`
- guard admin → form `is_active`（布林，必填）→ `410 Global user status is no longer supported`。

### `PUT /users/admin/{department_id}/{user_id}`
- guard admin → form `is_admin`（布林，必填）→ `410 Use /departments/{department_id}/admin instead`（`{department_id}` 代入實際值）。

---

## 4.4 Departments（router-level：認證 A）

### `GET /departments`
`200 [Department, …]`：**所有**系所（含私有），依 `sort_order ASC`。

### `GET /departments/status`
- `is_super` → `{"visible": 全部系所, "pending": []}`。
- 否則 `{"visible": [d | public ∨ approved], "pending": [d | ∃ m(u,d).status='PENDING']}`，皆依 `sort_order ASC`，元素為 `Department`。
- 快取：`dept-status` / 60。

### `GET /departments/visible`
`200 [Department, …]` = `visible_dept_ids(u)` 對應的系所，依 `sort_order ASC`。快取：`dept-visible` / 60。

### `GET /departments/{department_id}`
系所存在 → `200 Department`（**不檢查可見性**，私有系所也回傳）；不存在 → `404 Not Found`。

### `GET /departments/{department_id}/courses`
`require_viewable_department` → `200 [DeptCourse, …]`（順序未定義）。快取：`dept-courses` / 60。

### `GET /departments/{department_id}/bulletins`
`require_viewable_department` → `200 [Bulletin, …]`（順序未定義）。快取：`dept-bulletins` / 60。

### `GET /departments/{department_id}/visible-check`
`200 {"can_view": approved(u,d)}`（**只看 APPROVED membership**；公開系所或 super user 也可能是 `false`，見 Q-05）。系所不存在也回 `false`。

### `GET /departments/{department_id}/admin-check`
`200 {"is_admin": dept_admin(u,d)}`（不考慮 super user）。

### `POST /departments/{department_id}/join-request/send`
1. 系所不存在 → `404 Not Found`。
2. 已有 `m(u,d)`：`APPROVED` → `400 request has been approved`；其他 → `400 duplicate request`。
3. 新增 membership（`status='PENDING'`、`is_department_admin=false`）。併發重複送出 SHOULD 以交易/advisory lock 防止重複列。
4. 寄信給 `dept_admin_recipients(d)`（`06-integrations.md` §6.2，事件 E1）。
5. `200 Membership`。失效：`dept-status`。
- 公開系所也可以申請（Q-08）。

### `GET /departments/{department_id}/join-request`
guard admin → `200 [JoinRequest, …]`：該系所 `status='PENDING'` 的 membership（join users）。

### `GET /departments/{department_id}/members`
guard admin → `200 [Member, …]`：該系所 `status='APPROVED'` 的 membership（join users）。

### `PUT /departments/{department_id}/admin`
- guard admin → form `user_id`（必填）、`is_admin`（布林，必填）。
- `m(user_id, department_id)`（任何 status）不存在 → `404 Not Found`。
- 設定 `is_department_admin = is_admin`（值有改變時才更新 `updated_time`）；寄信（E3/E4，不論值是否改變）。
- `200 {"status":"success"}`。
- 失效：`user-admin`、`verify-token`、`posts`、`courses`、`course-detail`、`dept-courses`、`bulletins`、`dept-bulletins`。

### `PUT /departments/join-request/{department_id}/approve/{request_id}`
- guard admin（以路徑中的 `department_id`）。
- membership `id = request_id ∧ department_id = department_id` 不存在 → `404 Not Found`（防止以 A 系所權限核准 B 系所申請）。
- `status != 'PENDING'` → `400 request is not pending`。
- 設定 `status='APPROVED'`、`updated_time = now()`；寄信（E2）。
- `200 {"status":"success"}`。
- 失效：`dept-status`、`dept-visible`、`verify-token`、`posts`、`courses`、`course-detail`、`dept-courses`、`bulletins`、`dept-bulletins`，以及（F-05）`user-admin`。

---

## 4.5 Courses（router-level：認證 A）

### `GET /courses`
Query `skip`、`limit`（clamp）。`visible_dept_ids(u)` 為空 → `[]`；否則 `200 [CourseBrief, …]`（`department_id ∈ visible`，offset/limit，順序未定義）。快取：`courses` / 60。

### `GET /courses/{course_id}`
1. 課程不存在 → `404 Course not found`。
2. `require_viewable_department(u, course.department_id)`。
3. `200 {"course": Course, "posts": [CoursePost, …]}`，`posts` 只含 `status='APPROVED'`（順序未定義）。
- 快取：`course-detail` / 30。

### `POST /courses/{department_id}`
- guard admin → form `name`、`category`（必填）。
- 新增課程（不檢查系所是否存在）。`200 Course`。
- 失效：`courses`、`dept-courses`。

---

## 4.6 Bulletins（router-level：認證 A）

### `GET /bulletins`
Query `skip`、`limit`（clamp）。`200 [Bulletin, …]`（`department_id ∈ visible_dept_ids(u)`，順序未定義）。快取：`bulletins` / 60。

### `GET /bulletins/{bulletin_id}`
不存在 → `404 Bulletin not found`；`require_viewable_department`；`200 Bulletin`。快取：`bulletins` / 60。

### `POST /bulletins/{department_id}`
guard admin → form `title`、`content`（必填）→ 新增（不檢查系所是否存在，P-12）→ `200 Bulletin`。失效：`bulletins`、`dept-bulletins`。

---

## 4.7 Posts（router-level：認證 A）

### `GET /posts`
Query：`status`、`user_id`、`course_id`、`department_id`（字串，預設空）、`skip`、`limit`（clamp）。

1. 目前使用者不存在 → `401 User not found`。
2. `visible_dept_ids(u)` 為空 → `200 []`（此時不檢查 `status` 是否合法）。
3. 條件（AND）：
   - `post.department_id ∈ visible_dept_ids(u)`；
   - 非 super：`status='APPROVED'` ∨ `owner_id = u.id` ∨ `department_id ∈ {d | dept_admin(u,d)}`；
   - `status` 參數：`""` 不篩；`PENDING` → `status IS DISTINCT FROM 'APPROVED'`（**包含 REJECTED 與 NULL**）；`APPROVED`；`REJECTED`；其他 → `400 Invalid post status`；
   - `course_id`、`user_id`（= `owner_id`）、`department_id` 非空時精確比對；
   - 必須 inner join 到存在的 course。
4. 依 `create_time DESC` 排序，offset/limit。
5. `200 [PostListItem, …]`（`course_name` = course.name）。

不快取。

### `GET /posts/{post_id}`
1. Post 不存在或其 owner 使用者不存在（inner join users）→ `404 Not Found`。
2. 目前使用者不存在 → `401 User not found`。
3. `post.department_id ∉ visible_dept_ids(u)` → `404 Not Found`。
4. `status != 'APPROVED'`（含 NULL）且 `!can_view_unapproved_post(u,p)` → `404 Not Found`。
5. `200 PostDetail`（`files` 順序未定義）。
- 快取：`posts` / 60。

### `POST /posts`
Multipart（或 urlencoded，無檔案）：

| 欄位 | 規則 |
| --- | --- |
| `title` | 必填，1..=256 字元 |
| `course_id` | 必填，1..=256 字元 |
| `department_id` | 選填，預設 `""`，≤256 |
| `content` | 選填，預設 `""`，≤10000 |
| `is_migrate` / `is_anonymous` | 布林，預設 false |
| `files` | 選填，可重複的檔案欄位 |

1. `payload.id` 假值 → `401 Invalid token: missing user id`。
2. 課程不存在 → `404 Course not found`；`course.department_id != department_id` → `400 Course does not belong to the selected department`；`require_viewable_department(u, department_id)`。
3. 檔案（依序處理每個檔案）：
   - 檔案數 > 10 → `413 Too many files; maximum is 10`（在讀任何檔案前）。
   - part `Content-Type` 不是**完全等於** `application/pdf` 或 `application/x-pdf` → `400 Only PDF files are allowed`。
   - 單檔 > 20 MiB（20 971 520 bytes）→ `413 A PDF exceeds the per-file size limit`。
   - 累計 > 50 MiB（52 428 800 bytes）→ `413 PDF uploads exceed the total size limit`（同一檔案同時超過兩者時，以單檔錯誤優先）。
   - 內容不以 `%PDF-` 開頭 → `400 Invalid PDF content`（向量：`vectors/upload_magic.json`）。
   - 實作 MUST 串流讀取並在超過限制時立即停止，不可整包載入後才檢查。
4. 建立 post：`status = 系所 is_public ? 'APPROVED' : 'PENDING'`，`owner_id = u.id`。
5. 每個檔案上傳 R2：key `{user_id}/{post_id}/{uuid4}`，並新增 `posts_files(url = {R2_FILE_PATH}/{key}, post_id)`。
6. 交易語意：任一上傳或 DB 寫入失敗 → 不留下 post/posts_files 列，並 best-effort 刪除已上傳的物件 → `500`。
7. 私有系所 → 寄信給 `dept_admin_recipients(d)`（E5）；寄信失敗不影響回應。
8. `200 {"status":"success","post_id":"<id>"}`。失效：`course-detail`、`posts`、`dept-courses`。

### `PUT /posts/status/{department_id}/{post_id}`
1. guard admin → form `status`（必填）。
2. `status ∉ {PENDING, APPROVED, REJECTED}` → `400 Invalid post status`。
3. Post `id = post_id ∧ department_id = department_id` 不存在 → `404 Post not found`。
4. 更新 `status`（值有改變時才更新 `updated_time`，`05-data-model.md` §5.3）並 commit。
5. 系所列不存在（`posts.department_id` 無 FK）→ `404 Not Found`（狀態**已寫入**、不寄信）。
6. `status == 'APPROVED'` 且 owner 存在 → 寄信給 owner（E6；即使原本就是 APPROVED 也會寄，Q-12）。
7. 回應 = 以呼叫者身分執行 `GET /posts/{post_id}` 的結果（`PostDetail`；owner 不存在時 `200 null`）。
- 失效：`posts`、`course-detail`、`dept-courses`。

### `DELETE /posts/{post_id}`
1. `payload.id` 假值 → `401 Invalid token: missing user id`。
2. Post 不存在 → `404 Post not found`。
3. 非 super 且 `!dept_admin(u, post.department_id)` → `403 Forbidden`（**post 作者本人也不能刪**，Q-04）。
4. 同一交易刪除 `posts_files` 與 `posts` 列；commit 後 best-effort 刪除 R2 物件（key = URL 去掉 `{R2_FILE_PATH}/` 前綴；失敗只 log）。
5. `200 {"status":"success"}`。失效：`posts`、`course-detail`、`dept-courses`。

---

## 4.8 Threads（無 router-level 認證；handler 內認證 B；時間格式 TS_Z；不快取）

共用：
- **visible course(c)**：課程不存在 → `404 Course not found`；`require_viewable_department(u, c.department_id)`。
- **visible thread(t)**：thread 不存在 → `404 Thread not found`；visible course(t.course_id)。
- **visible comment(c)**：comment 不存在 → `404 Comment not found`；visible thread(c.thread_id)。
- **thread moderator**：`t.owner_id = u.id` ∨ `is_super(u)` ∨ `dept_admin(u, course(t).department_id)`。

### `POST /threads/{course_id}`
- Multipart/urlencoded：`title`（1..=200）、`content`（1..=2000）、`is_anonymous`（布林，預設 false）、`image`（選填檔案）。
- 認證 B → visible course。
- 圖片：part `Content-Type` ∉ `{image/jpeg, image/png, image/webp, image/gif}` → `400 Unsupported image type`；> 5 MiB（5 242 880）→ `413 Image too large`；magic bytes 不符 → `400 Invalid image content`。
  Magic：`FF D8 FF`、`89 50 4E 47 0D 0A 1A 0A`、`GIF87a`、`GIF89a`、或 `RIFF` 且長度 ≥ 12 且 bytes[8..12] = `WEBP`（不要求 magic 與宣告的 content type 一致）。向量：`vectors/upload_magic.json`。
- 建立 thread（`like_count=0`），有圖片時 `image_url = {R2_FILE_PATH}/threads/{uuid4}` 並上傳到 key `threads/{uuid4}`；上傳失敗 MUST NOT 留下 thread 列 → `500`。
- `200 {"status":"success","thread_id":"<id>"}`。

### `GET /threads/{course_id}`
- Query `skip`、`limit`（clamp）。認證 B → visible course。
- `200 {"threads": [Thread, …], "total": <該課程 thread 總數>}`；threads 依 `create_time DESC`，只含 owner 存在者；`liked` 為布林。

### `GET /threads/detail/{thread_id}`
認證 B → visible thread → `200 Thread`（`liked` 布林）。owner 不存在 → `404 Thread not found`。

### `PUT /threads/{thread_id}`
- Form：`title`（選填，1..=200）、`content`（選填，1..=2000）。
- 認證 B → visible thread → `owner_id != u.id` → `403 Permission denied`（管理員也不行）。
- 更新提供的欄位（都沒提供則不變），有變更時 `updated_time = now()`。
- `200 {"status":"success","thread_id":"<id>"}`。

### `DELETE /threads/{thread_id}`
認證 B → visible thread → 非 moderator → `403 Permission denied` → 刪除（DB cascade 刪除 comments、likes）→ commit 後 best-effort 刪除圖片物件 → `200 {"status":"success"}`。

### `POST /threads/{thread_id}/like`
- 認證 B → visible thread → 切換按讚（以 row lock 序列化）：已讚 → 刪除 like、`like_count = max(like_count-1, 0)`；未讚 → 新增 like、`like_count + 1`。唯一鍵衝突（併發）→ rollback 後以 DB 現況回報。
- `200 {"thread": Thread（liked = 新狀態）, "liked": bool}`。

### `POST /threads/{thread_id}/comments`
- Form：`content`（1..=2000）、`is_anonymous`（布林）、`parent_comment_id`（選填；空字串視為未提供，F-01）。
- 認證 B → visible thread → parent 不存在 → `404 Parent comment not found`；parent 屬於其他 thread → `400 Parent comment does not belong to this thread`。
- 新增 comment（`like_count=0`）。`200 {"status":"success","comment_id":"<id>"}`。

### `GET /threads/{thread_id}/comments`
- Query `skip`、`limit`（clamp）。認證 B → visible thread。
- `200 [Comment, …]`：只含頂層（`parent_comment_id IS NULL`）、owner 存在者，依 `create_time ASC`；`liked` 布林；`reply_count` = DB 中直接回覆數；`replies` = `null`。

### `GET /threads/comments/{comment_id}`
- 認證 B → visible comment → 以 BFS 取得回覆樹：
  - 最多 10 層、最多 500 則回覆（不含根）。
  - 每層一次查詢：`parent_comment_id IN (上一層 ids)`，依 `(create_time ASC, id ASC)`，`LIMIT 剩餘額度`；只含 owner 存在者；已見過的 id 跳過。
  - 每個節點：`Comment`，`liked` 布林，`replies` = 其子節點陣列（依上述順序；葉節點為 `[]`），`reply_count` = **實際納入**的子節點數。
- 根留言的 owner 不存在（inner join users）→ `404 Comment not found`（有 FK，實務上不可達）。
- `200 Comment`（根節點）。測試向量：500 則同時間回覆時，依 id 排序取前 500。

### `PUT /threads/comments/{comment_id}`
- Form：`content`（選填，1..=2000）。
- 認證 B → visible comment → `owner_id != u.id` → `403 Permission denied` → 未提供 `content` → `400 Content is required` → 更新（內容有改變時才更新 `updated_time`）。
- `200 {"status":"success","comment_id":"<id>"}`。

### `DELETE /threads/comments/{comment_id}`
認證 B → visible comment → 非（comment owner ∨ super ∨ dept_admin(course 的系所)）→ `403 Permission denied` → 刪除（cascade 刪除子回覆與 likes）→ `200 {"status":"success"}`。

### `POST /threads/comments/{comment_id}/like`
認證 B → visible comment → 切換按讚（同 thread）→ `200 {"comment": Comment（liked = 新狀態、reply_count = null、replies = null）, "liked": bool}`。
