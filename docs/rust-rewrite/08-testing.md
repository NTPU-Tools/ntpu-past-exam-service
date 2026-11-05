# 08 — 測試策略與功能覆蓋

## 8.1 分層

| 層 | 位置 | 對象 | 誰提供 |
| --- | --- | --- | --- |
| L1 黑箱契約測試 | `contract-tests/tests/*.rs` | 任何實作的 HTTP 行為 + 共用狀態（DB、Redis key、R2 物件） | 本 branch（已對參考實作驗證） |
| L2 參考向量 | `contract-tests/vectors/*.json`、`fixtures/` | 無法黑箱觀察的純邏輯（IP 解析、LMS/Google 解析、bcrypt、magic bytes、email HTML…） | 本 branch（由參考實作程式碼產生） |
| L3 Rust 單元/整合測試 | `rust/tests/`（未來） | 讀取 L2 向量；以 mock 驗證外部依賴失敗情境（§8.5） | Rust 實作者 |
| L4 平行驗證 | staging | 正式資料快照 + 錄製流量重放比對 | P5 |

**驗收條件**：`CONTRACT_TARGET=rust` 下 L1 全綠、L3 涵蓋 §8.5 全部項目、`scripts/check_route_coverage.py` 通過。

## 8.2 執行

```bash
docker compose -f contract-tests/docker-compose.yml up -d --build --wait   # 隔離環境
cd contract-tests && cargo test                                            # 對 Python 參考實作
# Rust：以 contract-tests/contract.env 啟動後
CONTRACT_BASE_URL=http://127.0.0.1:58081 CONTRACT_TARGET=rust cargo test
python3 contract-tests/scripts/check_route_coverage.py                     # route 覆蓋檢查
```

參考實作上的結果（本 branch 建立時）：**160 passed / 0 failed**（含 4 個 `rust_only` 測試在 reference 模式下直接略過），多次連續執行穩定，約 5 秒。
Rust 實作（`rust/`）的結果：**160 passed / 0 failed**（含全部 `rust_only` 測試）＋ `cargo test` 的 **14 個向量驅動單元測試**全過。
以 `CONTRACT_TARGET=rust` 對參考實作執行時，刻意修正的測試如預期失敗：F-01（回 500）、F-08（物件為 `binary/octet-stream`）、F-14（`/openapi.json` 回 200）；F-05 單獨執行時失敗（快取未清），但在整套並行執行時可能因其他測試清除同一 namespace 而偶然通過——對 Rust 版的驗收仍然有效。

## 8.3 測試設計原則

- **完全黑箱**：只透過 HTTP 呼叫受測服務；資料以 SQL 直接寫入 PostgreSQL（系所沒有建立 API，這也是唯一可行方式）。
- **隔離**：每個測試建立自己的 UUID 命名資料；每個 `Ctx` 以 `X-Forwarded-For` 帶一個隨機 `198.18.0.0/15` 位址，rate limit 計數互不干擾；斷言只針對自己建立的資料（列表用「包含/不包含」或以 filter 縮小範圍，必要時 `get_all_pages` 走完分頁）。
- **快取**：凡是快取 endpoint 的寫入測試，都先「預熱」再寫入、再讀，以驗證失效而非僅驗證寫入。
- **rate limit**：測試開始前確保固定視窗剩餘時間足夠（`ensure_rate_window_headroom`），避免跨視窗造成不穩定。
- **可重複**：DB/MinIO 使用 tmpfs，`docker compose down -v` 即清空；測試本身不依賴空資料庫。
- **刻意修正**：以 `rust_only!()` 標記，只在 `CONTRACT_TARGET=rust` 執行。

## 8.4 覆蓋矩陣

### 8.4.1 Route 覆蓋（自動檢查）

`reference/routes.json`（自 FastAPI app 匯出，52 個 route）× `tests/*.rs` 的呼叫點：**52/52**。
未對應任何 route 的呼叫只有刻意的 404/405 探測（`GET /definitely/not/a/route`、`DELETE /ping`）。

### 8.4.2 行為覆蓋（規格章節 → 驗證方式）

「L1」欄為 `檔案::測試`；「L2/L3」表示由向量或 Rust 內部測試負責（理由在最後一欄）。

| 規格 | 行為 | L1 測試 | L2/L3 |
| --- | --- | --- | --- |
| 02 §2.1 | 布林/正整數環境變數解析 | — | `parsing.json`（需改環境變數重啟，黑箱不適合） |
| 02 §2.2 | 啟動、schema 檢查、healthcheck 子命令 | `platform::ping_*` | L3：`SCHEMA_CHECK` 三種模式、`healthcheck` exit code |
| 02 §2.3 | GET 2xx `no-store` | `platform::ping_returns_json_pong_with_no_store`、`successful_get_responses_are_marked_no_store` | |
| 02 §2.3 | 500 為 text/plain 且無 CORS | — | L3：注入 DB 錯誤（黑箱無穩定觸發點） |
| 02 §2.4 | 404 / 405 + `Allow`；不提供 `/openapi.json`（F-14） | `platform::unknown_route_is_json_404`、`wrong_method_is_json_405_with_allow_header`、`openapi_schema_is_not_exposed`（rust only） | |
| 02 §2.5 | CORS preflight 允許/拒絕、一般 request | `platform::cors_*`（3 個） | |
| 02 §2.6 | `{"detail"}`、422 格式 | `platform::validation_errors_use_the_fastapi_envelope`、`json_body_validation`；各 endpoint 的 `expect_detail` | |
| 02 §2.7.1 | 空字串≠缺少、code point 長度、布林 lax、同名欄位取最後、非 form content type | `platform::form_booleans_*`、`string_lengths_*`、`repeated_form_fields_*`、`form_endpoints_treat_other_content_types_*`；`threads::update_thread_by_owner_only`（`title=""`→422） | `parsing.json` |
| 02 §2.7.2 | query 整數 | `platform::validation_errors_*`、`courses_bulletins::list_pagination_is_clamped` | `parsing.json` 的 `query_int` |
| 02 §2.7.4 | 分頁嚴格/clamp | `users::list_users_pagination_is_strict`、`courses_bulletins::list_pagination_is_clamped`、`posts::list_filters_ordering_and_shape`、`threads::list_threads`、`comments::list_top_level_comments` | |
| 02 §2.7.5 | 認證先於驗證（router-level） | `auth_tokens::authentication_precedes_validation_on_router_auth_endpoints` | |
| 02 §2.8 | 兩種時間格式、UTF-8、欄位集合 | `platform::timestamps_*`、`non_ascii_*`；各測試的 `assert_keys` | |
| 02 §2.9 | log 不含敏感值 | — | L3：對 422 handler 與 request log 做 snapshot（對應現有 `tests/test_validation_log_security.py`） |
| 03 §3.1 | JWT 驗證（過期、簽章、alg none、exp/sub、aud、nbf、type） | `auth_tokens::invalid_signatures_and_claims_are_rejected`、`token_type_must_be_access` | `jwt.json`（Python 簽發 token 必須可驗證） |
| 03 §3.1.3 | Authorization header 解析 | `auth_tokens::missing_or_malformed_authorization_header` | |
| 03 §3.2 | 模式 A（id→sub→username，含 `id` 查無時退回 `sub`）、模式 B | `auth_tokens::deleted_user_is_rejected`、`early_tokens_resolve_*`、`unknown_id_claim_falls_back_to_sub`、`thread_endpoints_require_the_id_claim` | |
| 03 §3.3 | 不信任 claim、live DB、pending 不授權、super | `auth_tokens::forged_*`、`live_super_*`、`revoked_privileges_*`、`pending_admin_*`、`super_user_passes_admin_guard_*` | |
| 03 §3.4 | Session 註冊/輪替/重放/撤銷/legacy 遷移、Redis key 相容 | `auth_session::issued_tokens_*`、`cookie_refresh_rotates_*`、`rotated_refresh_token_*`、`refresh_rejections`、`legacy_bearer_*`、`refresh_logout_*`（4 個）、`sessions_created_by_another_implementation_*` | `legacy_refresh_window.json`；L3：Redis 不可用 → 503 |
| 03 §3.5 | Cookie 屬性、刪除、no-store、Origin/CSRF | `auth_session::login_returns_*`、`logout_endpoints_*`（2 個）、`cookie_refresh_requires_origin_and_csrf_header`、`cookie_takes_precedence_over_bearer`、`login_origin_checks` | `auth_origins.json`；L3：`AUTH_COOKIE_SECURE=true` 帶 `Secure` |
| 03 §3.6 | Rate limit（subject/global/IP/XFF/casefold/key 格式/不計被拒 Origin/欄位缺漏仍計數） | `auth_rate_limit::*`（11 個） | `client_ip.json`、`rate_limit.json`；L3：Redis 錯誤 fail-open + log 節流 |
| 03 §3.7.1 | 密碼登入成功/失敗、`grant_type` | `auth_session::login_failures`、`login_returns_*` | `bcrypt.json`、`lms_username.json` |
| 03 §3.7.2 | Google exchange | `auth_session::exchange_checks_origin_and_input_*`、`auth_rate_limit::exchange_shares_the_global_limit` | `google.json`、`google_school_id.json`；L3：以 mock server（`GOOGLE_*_URL`）跑完整 `/exchange` |
| 03 §3.7.3 | LMS 登入 | — | `lms.json`；L3：以 mock server（`LMS_BASE_URL`）跑完整 `/login`（含錯誤密碼不退回本地密碼 P-16） |
| 03 §3.7.5 | 使用者 upsert（F-02、`" "`、preferences） | `auth_session::create_user_by_super_user` | L3：LMS/Google upsert 與 note 保留 |
| 03 §3.7.6 | Session 簽發失敗 → 503 | — | L3：Redis 不可用 |
| 04 §4.1 | `/ping`、`/system-version` | `platform::ping_*`、`system_version_*` | |
| 04 §4.2 | `/verify-token` | `auth_tokens::verify_token_*`（2 個） | |
| 04 §4.2 | `/create_user` | `auth_session::create_user_*`（2 個） | |
| 04 §4.3 | `/users*` 全部 | `users::*`（15 個） | |
| 04 §4.4 | `/departments*` 全部 | `departments::*`（19 個） | |
| 04 §4.5–4.6 | `/courses*`、`/bulletins*`（含 P-12 不檢查系所存在） | `courses_bulletins::*`（9 個） | |
| 04 §4.7 | `/posts*`：建立/上傳限制/可見性/篩選/排序/審核（含系所列不存在時 commit 後 404）/刪除/R2 物件 | `posts::*`（21 個） | `upload_magic.json`；L3：上傳失敗 rollback + 清除物件、刪除物件失敗只 log |
| 04 §4.8 | threads：建立/圖片/列表/詳情/更新/刪除/按讚/併發 | `threads::*`（12 個） | `upload_magic.json`；L3：圖片上傳失敗不留列 |
| 04 §4.8 | comments：建立/錯誤/列表/樹/深度 10/上限 500/更新/刪除/按讚/可見性 | `comments::*`（11 個） | |
| 05 §5.2–5.3 | INSERT 預設值、`updated_time`（含 no-op 不更新） | 各建立測試檢查 DB 列；`users::update_me_*`、`departments::set_member_admin_flag`、`unchanged_admin_flag_does_not_touch_updated_time`、`posts::status_update_*`、`unchanged_status_does_not_touch_updated_time`、`threads::toggle_thread_like` | |
| 05 §5.4 | 交易/併發 | `threads::concurrent_likes_are_counted_exactly`（按讚併發）；`posts::upload_validation_rejects_without_side_effects` 只驗證「驗證失敗不留資料」 | L3：上傳中途失敗的 rollback、加入申請併發 |
| 06 §6.1 | R2 key / URL / 刪除 | `posts::uploaded_pdfs_are_stored_and_listed`、`delete_by_admin_*`；`threads::create_thread_with_image_*`、`delete_thread_removes_its_image` | |
| 06 §6.2 | Email 事件 E1–E6、主旨前綴、HTML | — | `email.json`；L3：mock SES（`AWS_SES_ENDPOINT`）驗證每個事件的收件者/主旨/內文、escape（F-03）、失敗不影響回應（F-04） |
| 06 §6.3 | 快取隔離與失效矩陣（只驗證黑箱**可觀察**的項目，見 `06` §6.3.3 說明） | `cache_invalidation::*`（6 個）、`departments::approve_join_request`、`join_request_send_refreshes_status_cache`、`courses_bulletins::create_*`、`posts::status_update_by_department_admin`、`delete_by_admin_*`（含 `dept-courses`） | L3：矩陣中不可觀察的 namespace 也有清除、Redis 錯誤時 fail-open、舊 prefix 清除 |
| 07 F-xx | 刻意修正 | `comments::empty_parent_comment_id_*`（F-01）、`cache_invalidation::approval_refreshes_departments_admin_cache`（F-05）、`posts::uploaded_pdf_has_pdf_content_type`（F-08）、`platform::openapi_schema_is_not_exposed`（F-14） | 其餘 F 項見 §8.5 |

### 8.4.3 既有 Python 測試的對應

`tests/*.py`（4 檔）的案例對應如下：可黑箱者移入 L1（例如 CSRF 矩陣、輪替重放、rate limit、資源授權、上傳限制、快取失效、500 則回覆上限）。
`test_create_post_reads_only_up_to_configured_file_limit` 的可觀察結果（超過單檔上限 → 413）由 `posts::upload_validation_rejects_without_side_effects` 驗證，「只讀到上限 + 1 byte」的串流行為屬 L3。
`test_membership_approval_invalidates_resource_caches` 檢查的是「呼叫了哪些 namespace 清除」（實作細節）；其中可觀察的部分在 L1，其餘（`posts`、`course-detail`、`dept-bulletins`）在黑箱下不可觀察，屬 L3。
其餘移入 L2/L3：
`test_refresh_fails_closed_when_session_store_is_unavailable`、`test_session_issuance_fails_closed_when_redis_is_unavailable`（Redis 故障）、`test_failed_object_upload_rolls_back_db_and_cleans_objects`（物件儲存故障）、`test_validation_handler_never_logs_request_values`（log 內容）、`test_secure_cookie_is_enabled_when_configured` / `test_legacy_refresh_bearer_is_disabled_by_default`（需不同環境變數）、`test_exchange_uses_same_cookie_and_json_contract`（需 mock Google）。

## 8.5 Rust 實作者必須補的 L3 測試清單

1. 以 `vectors/*.json` 驅動的表格測試：`client_ip`、`rate_limit`、`lms_username`、`bcrypt`、`upload_magic`、`google_school_id`、`legacy_refresh_window`、`auth_origins`、`parsing`、`jwt`、`email`、`lms`、`google`（每個檔案的每個 case）。
2. LMS 登入端到端（mock `LMS_BASE_URL`）：成功新建使用者（含 preferences）、成功更新既有使用者且**保留 note**（F-02）、錯誤密碼 400 且不退回本地密碼、頁面結構不符 → 500、逾時 → 500。
3. Google 登入端到端（mock `GOOGLE_TOKEN_URL` / `GOOGLE_USERINFO_URL`）：成功新建（`school_department=" "`）/ 更新（保留 `school_department` 與 note）、各 403 情境、缺 email/name（F-13）。
4. Redis 不可用：`/login` 回 503 且不設 cookie、`/refresh` 回 503、`/refresh/logout` 回 503、rate limiter 放行並節流 log、快取 fail-open。
5. R2 失敗：post 第 N 個檔案上傳失敗 → 無 post/posts_files 列且已上傳物件被刪除；thread 圖片上傳失敗 → 無 thread 列；刪除物件失敗 → API 仍 200。
6. Email（mock SES）：E1–E6 收件者（含 `s{username}@gm.ntpu.edu.tw` fallback、P-09 pending admin）、主旨前綴、HTML escape（F-03）、SES 失敗不影響回應（F-04）、未設定 SES 時略過。
7. `MAX_REQUEST_BODY_BYTES` 超過 → 413（F-07）；multipart 文字欄位 > 1 MiB → 400。
8. 500 回應格式與不帶 CORS header；log 不含 query/body/token（含 422 log 只有 type/loc）。
9. `SCHEMA_CHECK` 各模式、`healthcheck` 子命令、`HASH_KEY` 缺失時啟動失敗。
10. 時間格式序列化（微秒 0/非 0、非 UTC 輸入轉 UTC）。
11. `GET /posts` 在使用者沒有任何可見系所時回 `[]`，且此時不驗證 `status`（共用 DB 無法黑箱建構）。
12. 上傳串流讀取：單檔超過上限時最多只讀上限 + 1 byte 即中止；快取失效矩陣中黑箱不可觀察的 namespace 確實被清除。
13. 快取 key 隔離：同一使用者換發 token 後仍命中、不同使用者永不共用（L1 只驗證後者）。

## 8.6 已知的測試限制

- LMS fixtures 為依 parser 邏輯合成，非真實頁面（Q-17）；P3 時應以測試帳號錄製真實回應替換，再重新產生 `vectors/lms.json`。
- 契約環境啟用 legacy refresh 與 `AUTH_COOKIE_SECURE=false`；相反設定的行為由 L2/L3 驗證。
- 黑箱測試無法觀察 email 與 log；這兩者完全依賴 L3。
- `GET /posts` 在「完全沒有可見系所」時回 `[]` 的分支無法在共用資料庫中建構（其他測試建立的公開系所對所有人可見），由 L3 驗證。
