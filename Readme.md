# KOSPI200 선물/미니선물 시세 웹서비스

한국투자증권(KIS) Open API를 연동하여 KOSPI200 정규선물과 미니선물의 시세를 조회하고, 웹 브라우저에서 확인할 수 있는 웹서비스입니다.

---

## 📌 프로젝트 개요

| 항목 | 내용 |
|---|---|
| **목표** | KOSPI200 선물 / 미니선물 2종목의 시세·체결·분봉 데이터를 웹에서 조회 |
| **사용자** | 약 200명 (내부/제한 사용자) |
| **데이터 소스** | 한국투자증권 KIS Open API (REST + WebSocket) |
| **아키텍처** | Rust 백엔드 + React 프론트엔드 |
| **캐시** | Redis (KIS API 호출 제한 보호) |

---

## 🏗️ 시스템 구성



[사용자 브라우저]
│ (REST: 시세 조회, WebSocket: 실시간 푸시)
▼
[React 프론트엔드] (frontend/)
│
▼
[Rust 백엔드] (backend/) ── Axum + Tokio
│
├── Redis 캐시 (TTL 3초, KIS 호출 제한 보호)
│
├── KIS REST API (시세, 분봉)
│
└── KIS WebSocket (실시간 체결)


---

## 🛠️ 기술 스택

| 계층 | 기술 | 버전 |
|---|---|---|
| 백엔드 언어 | Rust | 1.100.0 |
| 백엔드 프레임워크 | Axum | 0.8 |
| 비동기 런타임 | Tokio | 1.x |
| HTTP 클라이언트 | reqwest (연결 풀 공유) | 0.12 |
| 캐시 | Redis | 7-alpine (Docker) |
| 프론트엔드 | React + TypeScript + Vite | - |
| 차트 | TradingView Lightweight Charts | - |
| 로깅 | tracing + tracing-appender | 0.3 / 0.2 |

---

## 📁 프로젝트 구조


rust/
├── history.md # 개발 히스토리 (타임라인)
├── README.md # 이 파일
│
├── backend/ # Rust 백엔드
│ ├── src/
│ │ ├── main.rs # 진입점
│ │ ├── kis.rs # KIS API 클라이언트 (토큰, 시세, 분봉)
│ │ ├── kis_ws.rs # KIS WebSocket (실시간 체결)
│ │ ├── master.rs # 종목 마스터 다운로드/파싱
│ │ ├── logging.rs # 로그 시스템
│ │ ├── core/
│ │ │ ├── mod.rs
│ │ │ ├── state.rs # AppState (공유 상태)
│ │ │ └── candles.rs # 캔들 저장소 + N분봉 파생
│ │ ├── cache/
│ │ │ ├── mod.rs
│ │ │ └── redis_cache.rs # Redis 시세 캐시
│ │ └── server/
│ │ ├── mod.rs
│ │ ├── router.rs # Axum 라우터
│ │ └── handlers.rs # HTTP + WebSocket 핸들러
│ ├── logs/ # 일별 로그 (yyyy-mm-dd.log)
│ ├── .env # 환경 변수 (Git 제외)
│ ├── .token # KIS 토큰 캐시 (Git 제외)
│ └── Cargo.toml
│
└── frontend/ # React 프론트엔드
├── src/
│ ├── App.tsx
│ ├── App.css
│ ├── index.css
│ ├── PriceCard.tsx # 시세 카드
│ ├── CandleChart.tsx # 캔들 차트
│ └── types.ts
└── package.json



---

## 🚀 실행 방법

### 1. 사전 준비

- **Rust** 1.100.0 이상
- **Node.js** 22.x 이상
- **Docker Desktop** (Redis 실행용)
- **KIS API 키** (한국투자증권 Open API 포털에서 발급)

### 2. Redis 실행

Redis는 백엔드 실행 전에 반드시 실행되어 있어야 합니다. 현재 백엔드는 `127.0.0.1:6379`에 Redis가 열려 있다고 가정합니다.

**Docker Desktop UI로 실행하는 방법**

1. Windows 시작 메뉴에서 **Docker Desktop**을 실행합니다.
2. Docker Desktop이 완전히 시작될 때까지 기다립니다.
3. 왼쪽 메뉴에서 **Containers**를 엽니다.
4. Redis 컨테이너(기본 이름: `stock-redis`)를 찾습니다.
5. 해당 컨테이너의 **Start(▶)** 버튼을 눌러 실행합니다. 상태가 Running인지 확인합니다.
6. 백엔드를 실행합니다.

> Docker Desktop을 켜는 것만으로 Redis 컨테이너가 항상 실행되는 것은 아닙니다. 컨테이너가 중지 상태라면 UI에서 Start를 눌러야 합니다. `--restart unless-stopped` 옵션으로 생성한 컨테이너는 Docker가 시작될 때 자동 재시작되지만, 수동 중지한 컨테이너는 다시 시작해야 합니다.

**최초 1회 컨테이너 생성(아직 Redis 컨테이너가 없는 경우)**

```powershell
docker run -d --name stock-redis --restart unless-stopped -p 6379:6379 redis:7-alpine
```

**실행 확인(선택 사항)**

```powershell
docker exec -it stock-redis redis-cli ping
# 정상 응답: PONG

# 또는 Windows PowerShell에서 포트 확인
Test-NetConnection 127.0.0.1 -Port 6379
# TcpTestSucceeded : True
```

Redis가 실행되지 않은 상태에서 `cargo run`을 실행하면 연결 거부 오류(Windows의 경우 OS error 10061)가 발생할 수 있습니다.

### 3. 환경 변수 설정


```powershell
cd backend
Copy-Item .env.example .env







