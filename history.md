
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



