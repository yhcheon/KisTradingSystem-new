
# 프로젝트 개발 히스토리

KOSPI200 선물/미니선물 시세 웹서비스 개발 진행 기록.
각 단계의 목적, 변경 파일, 검증 결과를 기록한다.

---

## 📦 프로젝트 구성

| 구성 요소 | 경로 | 기술 스택 |
|---|---|---|
| 백엔드 | `backend/` | Rust + Axum + Tokio |
| 프론트엔드 | `frontend/` | React + TypeScript + Vite |
| 캐시 | Docker | Redis 7-alpine |
| 로그 | `backend/logs/` | tracing + LogExpert |

---

## 🗓️ 타임라인

### 2026-11-13

#### [백엔드] 1. 프로젝트 초기 환경 구축

- **목적**: Rust 백엔드 + React 프론트엔드 개발 환경 준비
- **설치**: Rust 1.100.0, Node.js v22.23.5, npm 10.9.9, VS Code + rust-analyzer
- **Docker/Redis**: Docker Desktop 설치, `stock-redis` 컨테이너 실행 (포트 6379)
- **프로젝트 생성**:
  - `cargo new backend` (Rust + Axum)
  - `npm create vite@latest frontend -- --template react-ts`

#### [백엔드] 2. Redis 연결 확인

- **파일**: `backend/src/main.rs`
- **내용**: Redis PING 테스트
- **결과**: `Redis PING → PONG` ✅

#### [백엔드] 3. KIS OAuth 토큰 발급 및 캐시

- **파일**: `backend/src/kis.rs`, `backend/.env`
- **내용**:
  - `/oauth2/tokenP` 엔드포인트로 접근토큰 발급
  - `.token` 파일에 토큰 + 발급시각 저장
  - 23시간 이내 재사용 (24시간 만료 안전 마진)
- **결과**:
  - 첫 실행: `KIS token issued and cached` ✅
  - 두 번째 실행: `KIS token loaded from cache` ✅

#### [백엔드] 4. 종목 마스터 다운로드 및 파싱

- **파일**: `backend/src/master.rs`
- **내용**:
  - `https://new.real.download.dws.co.kr/common/master/fo_idx_code_mts.mst.zip` 다운로드
  - zip 압축 해제 + EUC-KR → UTF-8 변환
  - `info_type=1`(정규선물), `info_type=B`(미니선물)만 필터링
  - 현재 날짜 기준 최근월물 자동 선택 (`pick_nearest`)
- **결과**:
  - 총 13개 선물 종목 파싱 ✅
  - 정규선물 최근월물: `F 202612 (A01612)` ✅
  - 미니선물 최근월물: `미니F 202612 (A05612)` ✅

#### [백엔드] 5. 선물 시세 조회 (KIS API)

- **파일**: `backend/src/kis.rs`
- **내용**:
  - `/uapi/domestic-futureoption/v1/quotations/inquire-price` 엔드포인트
  - TR ID: `FHMIF10000000`
  - 응답 구조: `output1`(시세), `output2`(지수 정보)
- **필드 매핑**:
  - `futs_prpr` (선물 현재가)
  - `futs_prdy_vrss` (전일 대비)
  - `futs_prdy_ctrt` (전일 대비율)
  - `acml_vol` (누적 거래량)
- **결과**:
  - 결과코드 0 (정상) ✅
  - 현재가 822.35, 전일대비 -5.15 (-0.62%), 거래량 114342 ✅

#### [백엔드] 6. 로그 시스템 구축

- **파일**: `backend/src/logging.rs`
- **내용**:
  - `tracing` + `tracing-subscriber` + `tracing-appender`
  - `logs/yyyy-mm-dd.log` 일별 로테이션
  - `RUST_LOG` 환경변수로 레벨 제어 (기본: debug)
  - 콘솔 출력 제거 (파일 전용)
- **주의**: `tracing-subscriber`에 `features = ["env-filter"]` 필요
- **결과**: `logs/2026-11-13.log` 생성 및 기록 ✅

#### [백엔드] 7. Redis 시세 캐시

- **파일**: `backend/src/cache/redis_cache.rs`, `backend/src/cache/mod.rs`
- **내용**:
  - `CachedPrice` 구조체 (코드, 이름, 현재가, 전일대비, 거래량, 캐시시각)
  - Redis 키: `price:{종목코드}` (예: `price:A01612`)
  - TTL 3초 (KIS REST 호출 제한 초당 20건 보호)
- **결과**:
  - 첫 조회: `[CACHE MISS]` → KIS API 호출 → `[CACHE SET]` ✅
  - 3초 이내 재조회 시 `[CACHE HIT]` 예정

---

## 🛠️ 개발 도구

| 도구 | 용도 |
|---|---|
| VS Code + rust-analyzer | Rust 코드 편집 |
| Docker Desktop + Redis 7-alpine | Redis 캐시 |
| LogExpert | 로그 파일 실시간 모니터링 (한글 깨짐 없음) |

---

## 📌 참고 사항

- KIS REST API 호출 제한: 초당 약 20건
- KIS WebSocket은 별도 `approval_key` 필요 (추후)
- 선물 시세는 평일 09:00~15:35 정규장에 실시간 갱신
- 종목코드는 월물 변경 시 자동 갱신 (마스터 파싱)

---

## 📋 다음 단계

### 백엔드
- [ ] Axum API 엔드포인트 (`/api/prices`, `/api/health`)
- [ ] WebSocket 실시간 푸시
- [ ] 분봉 차트 데이터

### 프론트엔드
- [ ] React 프로젝트 초기 설정
- [ ] 시세 대시보드
- [ ] 분봉 차트



#### [백엔드] 8. Axum API 엔드포인트 추가

- **파일**:
  - `backend/src/core/mod.rs`, `backend/src/core/state.rs` (신규)
  - `backend/src/server/mod.rs`, `backend/src/server/router.rs`, `backend/src/server/handlers.rs` (신규)
  - `backend/src/main.rs` (정리: 시세 테스트 제거, AppState 구성 후 서버 시작)
- **내용**:
  - `AppState`: 토큰, Redis 연결, 정규/미니 최근월물 공유
  - `GET /api/health`: 서버 상태 + 종목명
  - `GET /api/prices`: 정규선물 + 미니선물 시세 (캐시 우선, TTL 3초)
  - 실패 시 `{ ok: false, error }` 형태로 부분 실패 허용
- **결과**:
  - `/api/health` → `{ status: "ok", regular: "F 202612", mini: "미니F 202612" }` ✅
  - `/api/prices` → 두 종목 시세 JSON 반환 ✅
  - 3초 이내 재요청 시 캐시 히트 (KIS 호출 없음) 예정

### 백엔드
- [x] Axum API 엔드포인트 (`/api/prices`, `/api/health`)
- [ ] WebSocket 실시간 푸시
- [ ] 분봉 차트 데이터


#### [프론트엔드] 2. 시세 카드 5개 필드 표시

- **파일**:
  - `backend/src/kis.rs` (FuturesPriceOutput1에 futs_oprc/futs_hgpr/futs_lwpr/futs_prdy_clpr 추가)
  - `backend/src/cache/redis_cache.rs` (CachedPrice에 prev_close/open/high/low 추가)
  - `backend/src/server/handlers.rs` (fetch_one에서 새 필드 매핑)
  - `frontend/src/types.ts` (필드 추가)
  - `frontend/src/PriceCard.tsx` (5개 필드 그리드 표시)
  - `frontend/src/App.tsx` (3초 폴링 제거, 최초 1회만 조회)
  - `frontend/src/App.css` (그리드 스타일)
- **내용**:
  - 폴링 제거 → WebSocket 실시간 푸시로 전환 예정
  - 전일종가/시가/고가/저가/현재가 5개 필드 표시
- **결과**:
  - 브라우저에서 2종목 × 5필드 표시 ✅
  - **HTS 실제 수치와 일치 검증 완료** ✅


### 프론트엔드
- [x] React 프로젝트 초기 설정
- [x] 시세 대시보드 (5개 필드)
- [ ] WebSocket 실시간 푸시
- [ ] 분봉 차트


#### [백엔드] 9. KIS WebSocket 실시간 체결 수신

- **파일**:
  - `backend/src/kis.rs` (get_approval_key 추가)
  - `backend/src/kis_ws.rs` (신규, H0IFCNT0 WebSocket 클라이언트)
  - `backend/src/main.rs` (KIS WS 백그라운드 시작, AppState에 ws_tx 추가)
  - `backend/Cargo.toml` (futures-util, tokio-tungstenite 추가)
- **내용**:
  - `approval_key` 발급 → KIS WS 접속
  - 정규선물/미니선물 2종목 구독 (`tr_type: "1"`)
  - 수신한 체결을 `broadcast` 채널로 중계
  - 5초 후 자동 재연결 (loop)
- **결과**:
  - KIS WS 연결 + 구독 `SUBSCRIBE SUCCESS` ✅
  - PING/PONG으로 연결 유지 확인 ✅
  - 실시간 `[WS]` 로그는 장중에만 발생 (야간장은 코드 다름)

#### [백엔드] 10. Axum WebSocket 엔드포인트 (/ws)

- **파일**:
  - `backend/src/core/state.rs` (AppState에 ws_tx 추가)
  - `backend/src/server/handlers.rs` (ws_handler 추가, broadcast 구독 → 브라우저 전송)
  - `backend/src/server/router.rs` (/ws 라우트)
- **내용**:
  - `broadcast::Sender<RealtimeTick>`를 AppState로 공유
  - 브라우저 연결 시 `subscribe()`로 수신
  - JSON `{ type: "tick", code, price, change, ... }` 형태로 전송
- **결과**:
  - 브라우저 콘솔 `✅ connected` ✅
  - 장중 체결 시 `tick: {...}` 수신 예정

#### [프론트엔드] 3. WebSocket 실시간 갱신

- **파일**:
  - `frontend/src/types.ts` (RealtimeTick 타입)
  - `frontend/src/App.tsx` (WebSocket 연결 + 상태 갱신)
- **내용**:
  - 최초 REST 조회 → WebSocket 연결
  - 체결 수신 시 해당 종목의 현재가/전일대비/거래량만 갱신
  - 시가/고가/저가/전일종가는 최초 값 유지
  - 상단에 `실시간: 연결됨` 상태 표시
- **결과**:
  - 장중 체결 시 카드 실시간 갱신 예정

  ### 백엔드
- [x] Axum API 엔드포인트
- [x] KIS WebSocket 실시간 체결 수신
- [x] Axum WebSocket 엔드포인트 (/ws)
- [ ] 분봉 차트 데이터

### 프론트엔드
- [x] React 프로젝트 초기 설정
- [x] 시세 대시보드
- [x] WebSocket 실시간 푸시
- [ ] 분봉 차트




#### [백엔드] 11. KIS WebSocket PINGPONG 응답 처리

- **파일**: `backend/src/kis_ws.rs`
- **내용**:
  - KIS가 보내는 `PINGPONG` JSON을 받은 그대로 되돌려보내도록 수정
  - `tokio::sync::mpsc` 채널로 수신 루프와 응답 전송을 분리
  - `handle_text`에서 `PINGPONG` 감지 시 `pong_tx`로 원본 전달
- **결과**:
  - PINGPONG 10초 간격 수신 + 응답 전송 ✅
  - **연결이 8분 이상 유지** (이전에는 1~2분마다 끊김) ✅

#### [백엔드] 12. `reqwest::Client` 연결 풀 공유

- **파일**:
  - `backend/src/core/state.rs` (`http_client` 필드 추가)
  - `backend/src/main.rs` (앱 시작 시 1회 생성)
  - `backend/src/kis.rs` (모든 함수가 `&reqwest::Client` 매개변수 받도록 변경)
  - `backend/src/master.rs` (`download_master_text(&client)` 매개변수 추가)
  - `backend/src/server/handlers.rs` (`state.http_client` 전달)
- **내용**:
  - `reqwest::Client::new()`를 매 호출마다 생성하던 것을 앱 시작 시 1회로 변경
  - `pool_idle_timeout(90초)` 설정
- **결과**:
  - `starting new connection` 최초 1회만 발생 ✅
  - 이후 `reuse idle connection`으로 TCP/TLS 연결 재사용 ✅
  - KIS API 호출 성능 개선 + 포트 고갈 방지 ✅

#### [프론트엔드] 5. 레이아웃 좌우 2단 + 차트 영역 확장

- **파일**:
  - `frontend/src/App.css` (좌 280px + 우 minmax(0, 1fr))
  - `frontend/src/index.css` (Vite 기본 `#root width: 1126px` 제거)
  - `frontend/src/App.tsx` (좌: 시세 카드 세로, 우: 차트)
  - `frontend/src/CandleChart.tsx` (`ResizeObserver` + `Date.UTC` 시간 변환)
- **내용**:
  - `#root`의 고정 너비를 제거하고 화면 전체 사용
  - `.layout`의 `align-items: start` 제거 → 차트가 우측 꽉 채움
  - `ResizeObserver`로 창 크기 변화에 차트 자동 반응
  - 시간대 문제 해결 (`Date.UTC`로 KST → UTC 변환)
- **결과**:
  - 차트가 우측 넓은 영역을 사용 ✅
  - X축 시간이 08:45~15:45로 정상 표시 ✅

### 백엔드
- [x] Axum API 엔드포인트
- [x] KIS WebSocket 실시간 체결 수신
- [x] Axum WebSocket 엔드포인트 (/ws)
- [x] KIS WebSocket PINGPONG 응답 처리
- [x] reqwest::Client 연결 풀 공유
- [ ] 2분/5분봉 조합
- [ ] 장중 실시간 1분봉 누적

### 프론트엔드
- [x] React 프로젝트 초기 설정
- [x] 시세 대시보드 (5개 필드)
- [x] WebSocket 실시간 푸시
- [x] 전일 1분봉 차트
- [x] 좌우 2단 레이아웃
- [ ] 2분/5분봉 전환


#### [백엔드] 13. CandleStore 도입 + N분봉 파생

- **파일**:
  - `backend/src/core/candles.rs` (신규, CandleStore + aggregate)
  - `backend/src/core/state.rs` (candles 필드 추가)
  - `backend/src/main.rs` (전일 1분봉 로드 → CandleStore 파생)
  - `backend/src/server/handlers.rs` (`/api/candles`에 interval 파라미터)
- **내용**:
  - 1분봉 원본 + `2m/5m/10m/30m/60m` 파생 계산
  - 그룹 키 = `(날짜, 분 / N)` 방식으로 시간 경계 정확히 맞춤
  - 백엔드 시작 시 1회 계산 → 메모리 저장
  - 요청 시 메모리에서 복사만 (계산 없음)
- **결과**:
  - `cargo test` 3개 통과 ✅ (`aggregate` 정확성 검증)
  - 200명 동시 요청 시 응답 지연 1ms 이하 예상 ✅

#### [프론트엔드] 6. 분봉 선택 버튼 + 차트 색상 변경

- **파일**:
  - `frontend/src/App.tsx` (interval 상태 + 버튼)
  - `frontend/src/CandleChart.tsx` (interval prop + 색상)
  - `frontend/src/App.css` (버튼 스타일)
- **내용**:
  - 차트 상단에 `1분/2분/5분/10분/30분/60분` 버튼
  - 클릭 시 `/api/candles?interval=5m` 재조회
  - **차트 색상 변경**: 배경 검정(`#000000`), 양봉 초록(`#66bb6a`), 음봉 빨강(`#b71c1c`)
- **결과**:
  - 버튼 클릭 시 차트 봉 개수 즉시 변화 ✅
  - 1분(411건) → 5분(83건) → 60분(7건) ✅

  ### 백엔드
- [x] Axum API 엔드포인트
- [x] KIS WebSocket 실시간 체결 수신
- [x] Axum WebSocket 엔드포인트 (/ws)
- [x] KIS WebSocket PINGPONG 응답 처리
- [x] reqwest::Client 연결 풀 공유
- [x] CandleStore + N분봉 파생
- [ ] 장중 실시간 1분봉 누적

### 프론트엔드
- [x] React 프로젝트 초기 설정
- [x] 시세 대시보드 (5개 필드)
- [x] WebSocket 실시간 푸시
- [x] 전일 1분봉 차트
- [x] 좌우 2단 레이아웃
- [x] 분봉 선택 버튼 (1~60분)
- [x] 차트 색상 커스텀 (검정 배경, 초록/빨강)


#### [백엔드] 14. CandleStore 날짜별 관리 + 전일/당일 로드

- **파일**:
  - `backend/src/core/candles.rs` (`CandleStore`를 날짜별 `BTreeMap`으로 확장)
  - `backend/src/main.rs` (전일 + 당일(장중) 1분봉 로드)
  - `backend/src/server/handlers.rs` (`/api/candles?date=yesterday|today|both`)
- **내용**:
  - `HashMap<code, BTreeMap<date, HashMap<interval, Vec<Candle>>>>` 구조
  - 최대 2일치 보관 (3일째 되면 가장 오래된 날짜 제거)
  - 장중 여부 판단 (평일 08:45~15:45) → 당일 1분봉도 REST 로드
- **결과**:
  - 전일 411건 + 당일(장중이면) 로드 ✅
  - `/api/candles?date=both`로 어제+오늘 봉 반환 ✅

#### [프론트엔드] 7. 어제+오늘 가상 시간 이어붙임

- **파일**:
  - `frontend/src/App.tsx` (가상 시간 매핑 + `date`/`time` 필드)
  - `frontend/src/CandleChart.tsx` (`date`+`time` → UTC timestamp 변환)
- **내용**:
  - 어제 마지막 봉 + 1분 = 오늘 첫 봉으로 매핑
  - 빈 공간 없이 어제→오늘 이어붙임
  - X축 시각은 가상 시간(실제와 다를 수 있음)
- **결과**:
  - 토요일(장 마감) 기준 전일 데이터만 정상 표시 ✅
  - 장중에는 어제 + 오늘 봉이 이어서 표시 예정

  ### 백엔드
- [x] Axum API 엔드포인트
- [x] KIS WebSocket 실시간 체결 수신
- [x] Axum WebSocket 엔드포인트 (/ws)
- [x] KIS WebSocket PINGPONG 응답 처리
- [x] reqwest::Client 연결 풀 공유
- [x] CandleStore + N분봉 파생
- [x] CandleStore 날짜별 관리 + 전일/당일 로드
- [ ] 장중 실시간 1분봉 누적 (on_tick)

### 프론트엔드
- [x] React 프로젝트 초기 설정
- [x] 시세 대시보드 (5개 필드)
- [x] WebSocket 실시간 푸시
- [x] 전일 1분봉 차트
- [x] 좌우 2단 레이아웃
- [x] 분봉 선택 버튼 (1~60분)
- [x] 차트 색상 커스텀 (검정 배경, 초록/빨강)
- [x] 어제+오늘 가상 시간 이어붙임
- [ ] 장중 실시간 봉 갱신 (폴링)



#### [프론트엔드] 8. 카드 클릭 차트 전환 + 시세 상세 개선

- **파일**:
  - `frontend/src/App.tsx` (selected 상태, 카드 클릭 → 차트 종목 전환)
  - `frontend/src/PriceCard.tsx` (selected/onClick props, 전일 대비 계산)
  - `frontend/src/App.css` (카드 선택 강조, stats-list, 상단 바)
- **내용**:
  - 카드 클릭 → 해당 종목 차트 표시 (기본: 정규선물)
  - 선택 카드에 파란 테두리 + 그림자
  - 차트 제목: `정규선물(2026년 12월)` / `미니선물(2026년 12월)`
  - 상단 바: 제목(좌) + 상태(우) 한 줄 배치
  - 시세 상세: 전일종가/고가/시가/저가 + 전일 대비(삼각형+차이)
  - 대비 값 6자리 고정 (앞자리 0 → 공백으로 정렬)
- **결과**:
  - 카드 클릭 시 차트 즉시 전환 ✅
  - 시세 상세 정렬 깔끔 ✅


### 프론트엔드
- [x] 카드 클릭 차트 전환
- [x] 시세 상세 + 전일 대비 정렬
- [x] 상단 바 한 줄 배치
- [ ] 장중 실시간 봉 갱신 (폴링)


#### [백엔드] 15. WebSocket 체결 → CandleStore::on_tick 연동

- **파일**:
  - `backend/src/core/candles.rs` (`on_tick` 함수 추가)
  - `backend/src/kis_ws.rs` (`RealtimeTick.date` 추가, `Arc<Mutex<CandleStore>>` 전달 및 `on_tick` 호출)
  - `backend/src/main.rs` (`candle_store`를 `Arc<Mutex<>>`로 감싸서 WS에 전달)
- **내용**:
  - WebSocket 체결 수신 시 **모든 interval(1m~60m) 봉을 메모리에 실시간 누적**
  - 분이 바뀌면 이전 봉 확정 후 새 봉 시작 (그룹 키 = `분 / N` 방식)
- **결과**: 컴파일 성공 ✅ (월요일 장중 실시간 테스트 예정)

#### [프론트엔드] 9. 프론트엔드 자체 봉 조립 및 UI 개선

- **파일**:
  - `frontend/src/App.tsx` (WebSocket 체결로 현재 interval 봉 자체 조립, 시세 카드 데이터 로드)
  - `frontend/src/PriceCard.tsx` (제목+월물 표시, 종목 코드 줄 삭제)
  - `frontend/src/App.css` (CSS 변수 도입, 카드 제목/월물 스타일 추가)
- **내용**:
  - WebSocket 체결 수신 시 **프론트엔드가 현재 보고 있는 interval 봉을 직접 계산하여 갱신** (REST 폴링 없음)
  - 카드 UI 개선: "미니F 202612(A05612)" 줄 삭제, 제목 옆에 "2026년 12월" 표시
  - `:root`에 CSS 변수 정의로 테마 색상 일괄 관리
- **결과**:
  - 카드 UI 정상 표시 ✅
  - 분봉 변경 시 차트 정상 렌더링 ✅
  - 월요일 장중 실시간 봉 조립 테스트 예정

### 백엔드
- [x] WebSocket 체결 → CandleStore::on_tick 연동 (모든 interval 실시간 누적)

### 프론트엔드
- [x] 프론트엔드 자체 봉 조립 (WebSocket 체결)
- [x] 카드 UI 개선 (제목+월물)
- [x] CSS 변수 도입
- [ ] (월요일) 장중 실시간 봉 조립 최종 테스트

---

### 2026-11-15

#### [운영 문서] Redis GUI 실행 절차 및 거래일 차트 요구사항 기록

- **Redis 실행**: Windows Docker Desktop UI에서 Redis 컨테이너가 Running 상태인지 확인하고, 중지 상태라면 Start를 눌러야 백엔드가 정상 시작됨을 문서화.
- **기능 확인**: 사용자가 PR #22 범위의 Analysis 목록 조회 및 Admin 글 CRUD 기능 확인 완료를 보고함.
- **차트 날짜 요구사항(회의 필요)**:
  - 금요일 기준: 금요일 + 목요일
  - 토요일 기준: 금요일 + 목요일
  - 일요일 기준: 금요일 + 목요일
  - 달력상 어제/오늘이 아니라 최근 실제 거래일 2개를 표시하는 방향.
  - 초기 데이터 로딩 범위를 최근 거래일 2일치로 확대할지, 실제 거래 데이터가 존재하는 날을 역방향 탐색해 2개 거래일을 채울지 회의에서 결정.
- **상태**: 문서화만 수행. 차트 API 및 초기 캔들 로딩 코드는 변경하지 않음.



---

### 2026-11-18

#### [백엔드/프론트엔드] Backend 시작 준비 상태 및 재접속 제어 개선

- **목적**: Backend 초기화 중 Frontend가 API/WebSocket을 먼저 호출하면서 발생하는 Vite 프록시 `ECONNREFUSED`와 재접속 요청을 줄인다.
- **백엔드 변경**:
  - TCP Listener를 외부 서비스 초기화보다 먼저 바인딩한다.
  - `GET /api/health`를 초기화 전용 라우터에서 제공한다.
  - 초기화 중에는 `503 Service Unavailable` + `Retry-After`와 `status: starting`을 반환한다.
  - 초기화 성공 후 실제 API/WebSocket Router로 요청을 전달하고 `ready: true`를 반환한다.
  - 초기화 실패 시 Health Check를 유지하고 `status: failed`를 반환한다. 실제 API 요청은 준비 전까지 차단한다.
- **프론트엔드 변경**:
  - `useBackendHealth`가 Health Check를 주기적으로 확인한다.
  - Backend 준비 전에는 시세/캔들 REST 요청, 채팅 티켓/명령 요청, 두 WebSocket 연결을 시작하지 않는다.
  - Backend가 재시작되어 Health Check가 실패하면 진행 중인 요청/연결을 정리하고, 준비 완료 후 다시 연결한다.
- **호환성**:
  - 기존 `/api/health` 경로를 유지한다.
  - 준비 완료 응답은 기존 `status: "ok"`를 유지하고 `ready: true`를 추가한다.
- **검증 상태**:
  - 소스 정적 검토 완료.
  - `cargo check`, `cargo test`, `npm run build`, `npm run lint` 및 실제 Backend 재시작 시나리오는 개발 환경에서 실행하지 못했으므로 사용자 테스트가 필요하다.
