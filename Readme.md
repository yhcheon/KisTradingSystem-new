# KOSPI200 선물 시세 웹서비스

한국투자증권(KIS) Open API로 KOSPI200 정규선물·미니선물의 현재가, 실시간 체결, 분봉 차트를 제공하는 웹서비스입니다.
시세·차트 화면은 로그인 없이 볼 수 있고, 로그인 사용자는 분석 글과 뉴스 링크를 열람하며, 관리자는 콘텐츠를 관리할 수 있습니다.
익명 채팅(별칭 + 소셜 액션 명령어)과 주간/야간 시초 레인지 돌파 경보도 포함되어 있습니다.

> 이 README는 2026-11-20 기준 실제 소스 코드와 `cargo test` 결과를 기준으로 다시 작성한 것입니다.
> 2026-11-19 버전 이후 적용된 것: Graceful shutdown(JoinHandle 대기), 환경변수 한 번에 검증, 세션/레인지/종료 테스트 추가.

## 1. 시스템 구성

| 구분 | 스택 | 비고 |
| :--- | :--- | :--- |
| Frontend | React 19, TypeScript, Vite 8, `lightweight-charts` 5, `react-markdown` | `StrictMode` 사용 |
| Backend | Rust 2024 edition, Axum 0.8, Tokio | `backend/` |
| Market data | KIS REST API + WebSocket (`tokio-tungstenite`) | 선물 마스터 ZIP 파싱 포함 |
| Cache | **프로세스 내 인메모리 캐시** | Redis를 쓰지 않음. `PriceCache`가 주간/야간을 분리 보관, `CandleStore`가 분봉 보관 |
| Auth / Content | Supabase Auth + Supabase PostgREST(PostgreSQL) | 백엔드는 Service Role Key로 조회, 프론트는 Anon Key로 로그인 |
| Logging | `tracing` + 일별 파일 로테이션 | `backend/logs/YYYY-MM-DD.log`, 콘솔 출력 없음 |

브라우저는 Vite 개발 서버(5173)를 사용하고, Vite 프록시(`/api`, `/ws`)를 통해 Rust 백엔드(기본 `127.0.0.1:8080`)와 통신합니다.

### 데이터 흐름

```
KIS WebSocket (체결) ──┐
                       ├─> CandleStore (분봉) ──> /api/candles, /ws snapshot/tick
KIS REST (현재가/분봉) ─┤
                       └─> PriceCache (주간/야간 분리) ──> /api/prices, /ws
Supabase ──> 분석 글 / 뉴스 링크 / 관리자 권한 ──> /api/notes, /api/news, /api/admin/*
브라우저 ──> 익명 채팅 티켓 ──> /api/chat/ticket ──> /ws/chat (broadcast fan-out)
```

### 핵심 동작

- **시세:** 주간/야간 세션을 분리합니다. 같은 종목이라도 주간 가격과 야간 가격이 서로 덮어쓰지 않습니다.
- **분봉:** 1분봉을 기준으로 5분·15분·30분·60분봉 등을 집계합니다. `CandleStore`는 종목별 최대 2일치만 메모리에 유지합니다.
- **주간/야간 세션:** `SessionRangeMonitor`가 정규선물 틱만으로 주간(08:45~09:30), 야간(18:00~18:45) 시초 레인지를 수집합니다. 수집 중 WebSocket이 끊기면 해당 세션의 레인지는 무효 처리합니다. 수집이 끝난 뒤 고가/저가를 돌파·이탈하면 1회성 경보를 냅니다. 프론트는 경보를 차트 가격선으로 표시하고, `speechSynthesis`로 음성 안내합니다.
- **채팅:** `POST /api/chat/ticket`으로 30초 유효 티켓 + 익명 별칭을 발급받고, `/ws/chat?ticket=...`으로 연결합니다. 티켓은 1회성입니다. 메시지는 `tokio::sync::broadcast`로 fan-out되고, 접속자 수(presence)도 함께 브로드캐스트됩니다.
- **헬스체크:** 백엔드는 리스너를 먼저 열고 초기화가 끝나기 전에도 `/api/health`에 응답합니다. 초기화 중에는 `503 + Retry-After`, 완료 후에는 `200 + ready:true`를 반환합니다.
- **환경변수 검증:** 시작 시 `core/env.rs`가 필수 5개(`KIS_APP_KEY`, `KIS_APP_SECRET`, `SUPABASE_URL`, `SUPABASE_SERVICE_ROLE_KEY`, `SUPABASE_JWT_SECRET`)를 한 번에 검사합니다. 빠진 게 있으면 하나씩이 아니라 전부 나열하고 종료합니다.
- **Graceful shutdown:** Ctrl+C / SIGTERM 시 `Notify`가 4개 백그라운드 태스크(KIS WS, daily cleanup, chat cleanup, 야간 분봉 로더)에 전파되고, `server/runtime.rs`가 각 태스크를 최대 5초까지 기다린 뒤에 종료합니다. 종료 로그는 `백엔드 종료 완료`가 제일 마지막에 찍히는 순서가 정상입니다.

## 2. 주요 기능

- KOSPI200 정규선물·미니선물 현재가(주간/야간 분리 표시), 실시간 체결
- 분봉 차트(`lightweight-charts`) 및 분봉 주기 전환, REST + WS 체결 병합
- 주간/야간 시초 레인지 수집 상태 표시 + 고가 돌파 / 저가 이탈 경보 + 음성 안내
- Supabase Google OAuth 로그인 (팝업 방식, 부모 창에 `postMessage`로 완료 전달)
- 분석 글 목록 및 Markdown 상세 보기
- 관리자 분석 글 작성·수정·삭제, 관리자 뉴스 링크 등록·삭제
- 익명 채팅: 별칭 자동 발급, `/명령어` 응답, 소셜 액션(`/안녕` 등), 접속자 수 표시, 메시지 500자 제한, 연결당 10초 5개 rate-limit, IP당 티켓/연결 제한

## 3. 프로젝트 구조

```
KisTradingSystem-new/
  backend/
    src/
      main.rs         # 진입점: dotenv + 로깅 초기화 + runtime::serve_with_startup
      logging.rs      # 일별 파일 로그 (logs/YYYY-MM-DD.log)
      auth/           # JWT 검증 (Supabase ES256 + 레거시 HS256)
        jwt/          # claims, jwks, verify
      cache/          # PriceCache (인메모리, 주간/야간 분리)
      chat/           # 채팅 도메인: config, message, service, social
      core/           # bootstrap, state, market_data, market_session,
                      # session_range, time, cleanup, candles/, db/, env/
                      # env.rs = 필수 환경변수 한 번에 검증
        candles/      # CandleStore, aggregation, types
        db/           # SupabaseClient, query, mutations, cleanup
      kis/            # KIS API 클라이언트 (shim + 하위 모듈)
        auth/         # 토큰, approval key
        ws/           # WS 클라이언트, 프로토콜/틱 파싱
        candles.rs / quotes.rs / master.rs
      market/         # 과거 시세 로딩
        history/      # convert, pagination, time, 야간 분봉 로더
      master/         # 선물 마스터 파싱 / 최근월 선택
      server/         # router, runtime(종료 시 백그라운드 join), startup(헬스체크용), handlers/
        handlers/
          chat/       # 티켓 발급, WS 핸들러
          content/    # 분석 글 / 뉴스 / 관리자 / 인증
          market/     # prices, candles, WS, 분봉 진단
    Cargo.toml / Cargo.lock
    .env.example      # .env 템플릿 (실제 .env는 커밋 금지)
    .token            # KIS 토큰 캐시 (자동 생성, 커밋 금지)
    logs/             # 일별 로그 (커밋 금지)

  config/
    nicknames.json      # 채팅 별칭 단어 목록
    chat_commands.json  # 채팅 명령어 + 소셜 액션 템플릿

  frontend/
    src/
      main.tsx / App.tsx        # 앱 셸: 시세/채팅 패널 항상 마운트, 모달만 전환
      App.css / index.css
      CandleChart.tsx           # lightweight-charts 캔들 차트
      PriceCard.tsx             # 주간/야간 시세 카드
      candleColors.ts / types.ts
      components/   # AppHeader, AppModal
      features/
        admin/      # 관리자 글/뉴스 관리 + hooks/useAdminNotes
        analysis/   # 분석 글 목록/상세
        auth/       # hooks/useAuthSession, hooks/useAdminRole
        chat/       # ChatPage (티켓 발급 + WS + 재연결)
        news/       # 뉴스 링크 표시
        prices/     # PricesTab, hooks/usePrices, hooks/useMarketWebSocket,
                    # utils/candleUtils, types
      hooks/        # useBackendHealth (헬스체크 폴링)
      lib/          # supabase.ts (Anon Key), apiClient.ts (인증 fetch)
      pages/        # Admin, Analysis, Login
    vite.config.ts  # /api, /ws 프록시
    package.json

  docs/             # 세션 레인지 돌파 경보 등 설계 문서
  DESIGN.md / history.md
  Readme.md         # 이 파일
```

> 참고: `backend/src/kis.rs`, `master.rs`, `server/handlers.rs`는 실제 로직이 아니라 하위 모듈을 re-export하는 shim입니다. 중복 모듈이 아니니 삭제하지 마세요.

## 4. 사전 준비

- Rust toolchain (프로젝트 기준 1.100.0 이상)
- Node.js 22.x 이상 및 npm
- 한국투자증권 Open API 키 (앱 키 / 앱 시크릿)
- Supabase 프로젝트 및 Google OAuth 설정
- Docker는 필수가 아닙니다. Redis도 사용하지 않으므로 Redis 컨테이너가 필요 없습니다.

## 5. 환경 변수 설정

실제 비밀 키를 저장하는 `.env`, `.env.local`, `.token` 파일은 Git에 커밋하지 마세요.

### Backend

① 프로젝트 루트(또는 `backend/` 폴더)에서:

```powershell
cd backend
Copy-Item .env.example .env
```

`backend/.env`에 아래 값을 실제 환경에 맞게 입력합니다. 변수 이름과 설명은 `backend/.env.example`을 기준으로 합니다.

| 변수 | 설명 |
| :--- | :--- |
| `KIS_APP_KEY` / `KIS_APP_SECRET` | KIS Open API 앱 키 / 시크릿 |
| `KIS_ACCOUNT_NO` / `KIS_ACCOUNT_PROD_CD` | 계좌 정보 (기본 `01`) |
| `KIS_REST_DOMAIN` | 기본 `https://openapi.koreainvestment.com:9443` |
| `KIS_WS_DOMAIN` | 기본 `ws://ops.koreainvestment.com:21000` |
| `KIS_WS_RECONNECT_DELAY_SECS` / `KIS_WS_MAX_RECONNECT_ATTEMPTS` | KIS WS 재연결 설정 (`.env.example`에 있음) |
| `TOKEN_CACHE_FILE` | 기본 `./.token`, KIS 토큰 캐시 파일 경로 |
| `REDIS_URL` | `.env.example`에 남아 있으나 **현재 코드는 Redis를 사용하지 않음** |
| `BACKEND_BIND_ADDR` | 기본 `127.0.0.1:8080` |
| `LOG_LEVEL` / `LOG_DIR` | 기본 `info` / `./logs` |
| `CORS_ALLOWED_ORIGINS` | 허용 Origin 목록(쉼표 구분). 미설정 시 `localhost:5173` 등만 허용 |
| `CHAT_NICKNAMES_CONFIG` / `CHAT_COMMANDS_CONFIG` | 기본 `config/nicknames.json`, `config/chat_commands.json` |
| `TRUSTED_PROXY_IPS` | 신뢰할 프록시 IP 목록. 설정한 경우에만 해당 프록시의 `X-Forwarded-For`를 신뢰 |
| `SUPABASE_URL` / `SUPABASE_ANON_KEY` | Supabase URL / Anon Key |
| `SUPABASE_SERVICE_ROLE_KEY` | 백엔드 전용. **프론트엔드에 넣지 마세요** |
| `SUPABASE_JWT_SECRET` | 레거시 HS256 JWT 검증용. **백엔드 전용** |

채팅 별칭은 저장소 루트 `config/nicknames.json`에서, 명령 응답은 `config/chat_commands.json`에서 읽습니다.
두 설정 모두 백엔드 시작 시 한 번 읽으므로 변경하려면 재시작/재배포가 필요합니다. 설정이 없거나 유효하지 않으면 백엔드 시작이 실패합니다.

신뢰 프록시 뒤에서 운영할 때는 `TRUSTED_PROXY_IPS`를 설정하고, 프록시가 `X-Forwarded-For`를 안전하게 덮어쓰도록 구성하세요.

### Frontend

① 프로젝트 루트에서:

```powershell
cd frontend
Copy-Item .env.example .env.local
```

`frontend/.env.local`에 아래 값을 입력합니다.

| 변수 | 설명 |
| :--- | :--- |
| `VITE_PORT` | 기본 `5173` |
| `VITE_DEV_BACKEND_URL` | 기본 `http://127.0.0.1:8080` (Vite 프록시 타깃) |
| `VITE_SUPABASE_URL` | Supabase URL. **`.env.example`에는 없지만 `src/lib/supabase.ts`에서 필수** |
| `VITE_SUPABASE_ANON_KEY` | Supabase Anon Key (공개 키). **`.env.example`에는 없지만 필수** |

> `VITE_SUPABASE_*`가 없으면 프론트 시작 시 `Supabase 환경 변수가 없습니다` 오류로 중단됩니다.

## 6. 로컬 실행

각 프로세스는 별도의 PowerShell 창에서 실행합니다.

### 1) Backend

① Backend PowerShell (`backend/` 폴더)에서:

```powershell
cargo run
```

기본 로컬 주소는 `http://127.0.0.1:8080`입니다. 실제 바인딩 주소는 `BACKEND_BIND_ADDR` 설정을 따릅니다.
초기화 중에도 `/api/health`는 응답합니다.

### 2) Frontend

② Frontend PowerShell (`frontend/` 폴더)에서:

```powershell
npm install
npm run dev
```

브라우저에서 `http://localhost:5173`을 엽니다.

## 7. 검증 명령

명령은 해당 폴더에서 실행합니다.

Backend PowerShell (`backend/`):

```powershell
cargo check
cargo test
```

2026-11-20 기준 `cargo test` 결과: **100개 중 99개 통과, 1개 실패(flaky)**. 실패하는 1개는 `broadcasts_once_to_two_clients_including_sender_without_replay`로, 두 입장 메시지의 도착 순서(2ms 차이)를 고정으로 기대하는 기존 테스트의 타이밍 경합이며, 이번 변경과 무관합니다. 새로 추가된 테스트: `core/env` 3개, `core/market_session` 3개, `core/session_range` 3개, `server/runtime` 2개.

Frontend PowerShell (`frontend/`):

```powershell
npm run lint
npm run build
```

## 8. API 및 운영 참고

### HTTP API

| 메서드 / 경로 | 인증 | 설명 |
| :--- | :--- | :--- |
| `GET /` | 없음 | `backend ok` |
| `GET /api/health` | 없음 | 시작 중 `503`, 준비 완료 `200 + ready:true`, 초기화 실패 `503` |
| `GET /api/prices` | 없음 | 정규/미니 현재가 (주간/야간 분리) |
| `GET /api/candles?code=&interval=&date=` | 없음 | 분봉 조회. `date=both` 지원 |
| `POST /api/admin/market/minute-candle-check` | 관리자 | 1분봉 검증/재조회 (진단용) |
| `POST /api/chat/ticket` | 없음 | 채팅 티켓 + 익명 별칭 발급 (30초 유효, 1회성) |
| `GET /api/chat/commands` | 없음 | 채팅 명령어 목록 |
| `GET /api/chat/actions` | 없음 | 소셜 액션 목록 |
| `GET /api/notes` | 로그인 | 분석 글 목록 |
| `GET /api/notes/{id}` | 로그인 | 분석 글 상세 (Markdown) |
| `GET /api/news` | 로그인 | 뉴스 링크 목록 |
| `GET /api/admin/news` | 관리자 | 관리자용 뉴스 목록 |
| `POST /api/admin/news` | 관리자 | 뉴스 링크 등록 |
| `DELETE /api/admin/news` | 관리자 | 뉴스 링크 삭제 |
| `GET /api/admin/notes` | 관리자 | 관리자용 분석 글 목록 |
| `POST /api/admin/notes` | 관리자 | 분석 글 작성 |
| `PUT /api/admin/notes` | 관리자 | 분석 글 수정 |
| `DELETE /api/admin/notes` | 관리자 | 분석 글 삭제 |

인증이 필요한 API는 `Authorization: Bearer <Supabase JWT>` 헤더를 사용합니다.
백엔드는 Supabase ES256(JWKS) 및 레거시 HS256 JWT를 검증하고, issuer가 현재 Supabase 프로젝트와 일치하는지 확인합니다.
관리자 여부는 Supabase 세션 기준으로 판정합니다.

### WebSocket

| 경로 | 설명 |
| :--- | :--- |
| `GET /ws` | 시세 스트림. 연결 시 스냅샷(`snapshot` + 가격 + 레인지 상태) 전송, 이후 체결 `tick` 전송 |
| `GET /ws/chat?ticket=...` | 채팅 스트림. 티켓 소진 후 연결, 메시지 + presence 브로드캐스트 |

프론트 동작:

- 시세 WS는 화면 전환과 무관하게 항상 유지됩니다 (`App.tsx`에서 시세/채팅 패널을 항상 마운트).
- 시세/채팅 WS 모두 지수 백오프 재연결(1초 → 최대 30초)을 합니다.
- 시세 WS 스냅샷이 먼저 도착하면 REST `/api/prices` 응답이 실시간 값을 덮어쓰지 않습니다.
- 채팅은 최대 100개 메시지만 화면에 유지하고, 500자 초과 메시지는 전송하지 않습니다.

### 콘텐츠 보존·삭제 정책

- 분석 글·뉴스의 보존·삭제 정책, 인증 및 데이터 모델의 상세 설계는 `DESIGN.md`를 참고하세요.
- 백엔드는 매일 KST 07:00에 30일 지난 데이터를 삭제하는 백그라운드 cleanup을 실행합니다 (`core/cleanup.rs`, `core/db/cleanup.rs`).
- 주말/휴장일의 최근 거래일 차트 표시 정책은 `DESIGN.md`에 미확정 안건으로 기록되어 있습니다. 합의 전에는 날짜 조회 정책을 임의로 변경하지 않습니다.

### 알려진 고도화 과제 (2026-11-20 기준, 코드 확인됨)

다음은 버그가 아니라 다음 단계에서 다룰 과제입니다.

1. **채팅/WS 경로의 `unwrap()` 정리** — `chat/service.rs`, `handlers/chat/websocket.rs`에 `unwrap()`이 집중되어 있습니다. 외부 입력 경로에서 패닉이 나지 않도록 `Result` 처리로 교체하는 것이 1순위입니다.
2. ~~**Graceful shutdown 없음**~~ — **2026-11-20 완료.** `Notify` 전파 + `JoinHandle` 대기(태스크당 최대 5초)로, KIS WS 구독 해제 후 백엔드가 종료됩니다. 남은 한계: `Notify::notify_waiters()`는 저장되지 않는 신호라, 초기화 전에 온 종료 신호는 `runtime`이 따로 처리합니다. 완벽한 상태 유지를 원하면 `CancellationToken`/`watch`로 교체하는 것을 권장합니다. 야간 분봉 로더는 로드 1회분이 끝난 뒤 다음 대기 지점에서 종료됩니다.
3. **로그가 파일 전용** — `logging.rs`는 파일에만 쓰고 콘솔(stdout)에는 쓰지 않습니다. Docker/배포 환경에서는 stdout JSON 로그를 추가하는 것이 좋습니다.
4. **프론트 `.env.example` 누락** — `VITE_SUPABASE_URL`, `VITE_SUPABASE_ANON_KEY`가 `.env.example`에 없습니다. 실제 필수 변수이므로 템플릿에 추가하는 것이 좋습니다.
5. **백엔드 `.env.example` 잔재** — `REDIS_URL`이 남아 있으나 현재 코드는 Redis를 사용하지 않습니다. 혼란을 피하려면 템플릿에서 제거하거나 주석으로 명시하는 것이 좋습니다.
6. **프론트 루트 파일 정리** — `src/CandleChart.tsx`, `src/PriceCard.tsx`, `src/candleColors.ts`, `src/types.ts`가 `src/` 루트에 있습니다. `features/prices/`로 이동하면 구조가 더 깔끔해집니다.
7. **Flaky 테스트 1개** — `broadcasts_once_to_two_clients_including_sender_without_replay`가 두 입장 메시지의 도착 순서를 고정으로 기대해 가끔 실패합니다. 순서에 의존하지 않도록 테스트를 고치는 것이 좋습니다.


## 8-1. 최근 변경 (2026-11-20)

- **Graceful shutdown:** `core/bootstrap.rs`가 `initialize()`에서 `(AppState, Vec<JoinHandle>)`을 반환하고, `server/runtime.rs`가 서버 종료 후 백그라운드 태스크를 태스크당 최대 5초까지 기다립니다. `AppState`에 `JoinHandle`을 넣지 않은 이유: `AppState`는 `Clone`인데 `JoinHandle`은 `Clone`이 안 되기 때문입니다.
- **환경변수 한 번에 검증:** `core/env.rs` 신규. 시작 시 필수 5개를 한 번에 검사합니다.
- **테스트 추가:** `core/env` 3개, `core/market_session` 3개(빈틈·자정 넘김·겹침 금지), `core/session_range` 3개(수집 시작 전 무시·잘못된 입력 무시·야간 경계), `server/runtime` 2개(빠른 종료 대기·멈춘 태스크 타임아웃).
- **정리:** `candle_merge.rs`는 `CandleStore`에 이미 동등한 병합/집계 테스트가 있어 추가하지 않았습니다. 중복 로직 방지를 위함입니다.

## 9. 보안 주의사항

- `.env`, `.env.local`, `.token`, 서비스 키, JWT Secret, 계좌 정보는 저장소나 드라이브에 올리지 않습니다. 드라이브에 `.token`이 남아 있으면 삭제하세요.
- Supabase Service Role Key와 `SUPABASE_JWT_SECRET`은 백엔드에서만 사용합니다. 프론트에는 Anon Key만 넣습니다.
- KIS 토큰 파일(`.token`)에는 토큰 값 + 발행 시간이 기록되며, 프로그램이 24시간 경과 여부를 판단해 자동 재발급합니다. `.token`도 절대 Git에 올리지 않습니다.
- 뉴스는 제목과 링크 중심으로 관리하며 기사 본문 전문을 무단 저장하지 않습니다.
- 채팅은 익명이며, IP 기반 티켓/연결 제한과 메시지 rate-limit이 있습니다. `TRUSTED_PROXY_IPS`를 설정하지 않으면 `X-Forwarded-For`를 신뢰하지 않으므로, 프록시 뒤에서 운영할 때는 반드시 설정하세요.
- JWT 원문이나 서명값은 로그에 기록하지 않습니다.
