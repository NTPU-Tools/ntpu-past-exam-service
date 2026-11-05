# 07 — 現行怪異行為：保留、修正、待決定

分析參考實作時發現的非直覺行為分三類。**預設原則是保留**；只有 F 類允許（且要求）Rust 版不同。
待決定（Q 類）在產品負責人決定前一律依「保留」實作；若決定修改，請把該項移到 F 類並更新對應測試。

## P — 必須保留（前端或資料相容性可能依賴）

| 代號 | 行為 | 位置 |
| --- | --- | --- |
| P-01 | `GET /departments/{id}/visible-check` 只看 APPROVED membership，公開系所與 super user 也可能回 `false` | `departments/dependencies.py:53` |
| P-02 | `GET /departments/{id}/admin-check` 不把 super user 視為管理員 | `departments/dependencies.py:65` |
| P-03 | `GET /departments`、`GET /departments/{id}` 對任何登入者回傳私有系所的基本資料（加入申請流程需要） | `departments/router.py:33,126` |
| P-04 | Post 作者不能刪除自己的 post（只有該系所管理員或 super）；thread/comment 作者則可以刪自己的 | `posts/router.py:200` |
| P-05 | `GET /users/{自己的 id}` 回傳遮蔽版；只有 `/users/me` 回完整資料與偏好 | `users/router.py:57` |
| P-06 | 查不存在的使用者：一般使用者得 403、super 得 404 | `users/router.py:49` |
| P-07 | `GET /posts?status=PENDING` 包含 `REJECTED` 與 `NULL` | `posts/dependencies.py:105` |
| P-08 | `PUT /posts/status/...` 在 owner 不存在時回 `200 null` | `posts/router.py:175` |
| P-09 | 管理員通知信收件者包含「is_department_admin=true 但 status 仍為 PENDING」的成員 | `departments/dependencies.py:407` |
| P-10 | 公開系所也能送加入申請 | `departments/dependencies.py:212` |
| P-11 | 系所管理員可對該系所任何 membership（含 pending、含自己、含其他管理員）設定/取消管理員 | `departments/dependencies.py:300` |
| P-12 | `POST /courses/{department_id}`、`POST /bulletins/{department_id}` 不檢查系所是否存在（只有 super user 能對不存在的系所通過 guard） | `courses/router.py:69`、`bulletins/router.py:63` |
| P-13 | thread 端點與 `/verify-token`、`/refresh*`、`/logout` 先驗證輸入（422）再認證（401） | `thread/router.py` |
| P-14 | 兩種時間格式（`+00:00` 與 `Z`） | `02-runtime.md` §2.8 |
| P-15 | thread / comment 按讚會更新 `updated_time` | `thread/dependencies.py:415` |
| P-16 | 數字帳號（LMS 資格）登入失敗不會退回本地密碼 | `auth/router.py:231` |
| P-17 | `/create_user` 空字串帳密回 `400 Invalid input`（不是 422） | `auth/router.py:398` |
| P-18 | token 仍帶 `isu`、`adm` claim（不用於授權） | `auth/router.py:97` |
| P-19 | `/logout` 只刪 cookie 不撤銷（cookie Path 使其收不到 cookie） | `auth/router.py:382` |
| P-20 | thread 列表的 `total` 包含 owner 已不存在的 thread，但列表本身排除它們（目前 `threads.owner_id` 有 FK，實務上不可達；若日後移除 FK 需保留此語意） | `thread/dependencies.py:82` |
| P-21 | 設定管理員時即使值未改變也寄信；狀態設為 APPROVED 時即使原本已核准也寄信 | `departments/dependencies.py:300`、`posts/dependencies.py:317` |
| P-22 | `/system-version` 的 key 是 `GIT_SHA` | `main.py:231` |
| P-23 | Google 新建使用者的 `school_department` 是單一空白 `" "` | `auth/router.py:213` |
| P-24 | `GET /courses` 只回 `id, name, category`（response_model 過濾） | `courses/router.py:20` |
| P-25 | form 空字串視為「有值」並套用長度檢查（例如 thread `title=""` → 422，而非「未提供」） | FastAPI 0.115 行為 |

## F — Rust 版刻意修正

| 代號 | 參考實作 | Rust 版 | 測試 |
| --- | --- | --- | --- |
| F-01 | `POST /threads/{id}/comments` 帶 `parent_comment_id=""` → FK 違反 → 500 | 空字串視為未提供，建立頂層留言 | `comments::empty_parent_comment_id_creates_top_level_comment`（rust only） |
| F-02 | LMS/Google 登入更新使用者時把 `note` 清成 NULL（資料遺失） | 登入不修改 `note` | 單元測試（LMS/Google 需 mock） |
| F-03 | email 模板直接串接系所名稱、post 標題（HTML injection） | 代入值 HTML escape | 單元測試 |
| F-04 | 寄信同步執行於 request 中，部分錯誤會讓已 commit 的請求回 500 | 寄信於 commit 後背景執行，失敗只 log | 單元測試 |
| F-05 | 核准加入申請不清除 `user-admin` 快取（pending 時已被設為管理員者，最多 30 秒看不到管理頁） | 一併清除 | `cache_invalidation::approval_refreshes_departments_admin_cache`（rust only） |
| F-06 | 多個列表無 ORDER BY | 一律穩定排序（`(create_time, id)` 或 `sort_order` 後以 `id`） | 測試不依賴順序 |
| F-07 | request body 無上限（Starlette 先整包 spool 到磁碟） | 上限 `MAX_REQUEST_BODY_BYTES`，超過 `413 {"detail":"Request body too large"}` | 單元測試 |
| F-08 | 上傳物件未設 `Content-Type` | posts `application/pdf`、圖片用宣告型別 | `posts::uploaded_pdf_has_pdf_content_type`（rust only） |
| F-09 | handler 用 `payload.id`，與認證解析（可退回 `sub`）不一致 | 一律使用模式 A 解析出的使用者 | 系統簽發的 token 行為相同 |
| F-10 | `varchar(256)` 欄位超長 → DB 錯誤 500 | MAY 先驗證回 422 | 測試不依賴 |
| F-11 | 快取 key 用 `md5(Authorization)`、失效用 `KEYS` | 用 user id + generation counter | 黑箱不可見 |
| F-12 | 資料庫中 bcrypt hash 格式錯誤 → 500 | MAY 回 `400 incorrect username or password` | 單元測試 |
| F-13 | Google 回應缺 `email`/`name` → 500 | MAY 回 `403 Forbidden` | 單元測試 |
| F-14 | `GET /openapi.json` 公開完整 API schema（`/docs`、`/redoc` 已關閉，推測為疏漏） | 不提供，`404` | `platform::openapi_schema_is_not_exposed`（rust only） |
| F-15 | log：設定 Better Stack 時不寫本機 log、預設寫 stderr、422 log 的 `env` 誤用 `R2_BUCKET_NAME`、500 沒有 request log | 依 `02-runtime.md` §2.9 Rust 要求 | 單元測試 |
| F-16 | email 主旨在 `SERVICE_NAME != "PROD"` 時加「測試環境 - 」；正式環境實測 `SERVICE_NAME=oracle-production-coolify` → **每封信都有測試前綴** | `EMAIL_SUBJECT_PREFIX` 可覆寫；否則以 `SERVICE_NAME` 的 `prod`/`production` token 判斷正式環境 | `tests::email_rendering_vectors`（含 `is_production`） |
| F-17 | `GET /posts?user_id=X` 列出 X 的匿名 post，可反推作者（正式環境實測 3 筆） | `user_id` 篩選對非本人、非 super user 排除匿名 post | `posts::anonymous_posts_are_not_discoverable_through_the_user_filter`（rust only） |
| F-18 | legacy refresh bearer 遷移已在 2026-11-03 截止；正式環境 1600 個 session 全為 managed、flag 未設 | 移除 legacy 遷移與 `auth:legacy-refresh-used:*`；legacy 形狀回 401、logout 只刪 cookie | `auth_session::legacy_refresh_bearer_is_rejected_after_removal`、`refresh_logout_with_a_legacy_cookie_is_idempotent`（rust only） |

## Q — 待決定（預設保留現況）

| 代號 | 問題 | 建議 |
| --- | --- | --- |
| Q-03 | 寄給多位管理員的信放在同一個 `To:`，彼此看得到 email | 改為逐一寄送或用 BCC |
| Q-04 | P-04：作者不能刪自己的考古題 | 確認是否為刻意設計 |
| Q-05 | P-01：`visible-check` 語意 | 確認前端是否仍使用（目前前端程式碼未呼叫） |
| Q-07 | `is_migrate` 由客戶端任意設定（影響前端顯示作者） | 限 super user 可設定 |
| Q-08 | P-10：公開系所也可送加入申請 | 確認 |
| Q-09 | P-11：管理員可互相降級、可把 pending 成員設為管理員 | 確認 |
| Q-10 | 系所沒有 CRUD API（`is_public`、`sort_order` 只能改 DB） | 維持現況或另開需求 |
| Q-11 | `users_departments` 無唯一約束，可能已存在重複列 | P7 檢查資料後加 unique index |
| Q-12 | P-21：重複寄送通知 | 只在狀態實際改變時寄 |
| Q-13 | `/refresh`、`/create_user` 沒有 rate limit | 視需要加上 |
| Q-14 | thread 管理員可刪除但不能編輯他人 thread | 確認 |
| Q-15 | `/departments/{id}/visible-check`、`admin-check`、`/users/status/*`、`/users/admin/*`、`/logout` 等前端已不使用的 endpoint 是否保留 | 保留到 P7 後依 access log 決定 |
| Q-16 | 時間格式是否統一 | 前端 `parseUTC` 兩種都接受；統一需改測試，維持現況 |

> Q-01、Q-02、Q-06 已於 2026-11 以 oracle 正式環境驗證並轉為 F-16、F-17、F-18。
| Q-17 | LMS 頁面實際編碼與 HTML 結構需以真實帳號驗證（fixtures 為依 parser 邏輯合成） | P3 時以測試帳號錄製真實回應替換 fixtures |
| Q-18 | 系所管理員收到的 E1 信件連結使用 `ORIGIN`，未設定時參考實作會產生 `None/admin/...` | Rust 未設定 `ORIGIN` 時啟動 log warning |
