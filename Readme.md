# KOSPI200 선물 시세 웹서비스

한국투자증권(KIS) Open API를 이용해 KOSPI200 정규선물·미니선물의 현재가, 실시간 체결, 분봉 차트를 제공하는 웹서비스입니다. 로그인 사용자는 분석 글과 뉴스 링크를 열람하며, 관리자는 콘텐츠를 관리할 수 있습니다.

## 1. 시스템 구성

- Frontend: React 19, TypeScript, Vite
- Backend: Rust, Axum, Tokio
- Market data: 한국투자증권 KIS REST API 및 WebSocket
- Cache: Redis
- Authentication / content: Supabase Auth 및 PostgreSQL

브라우저는 Vite 개발 서버(5173)를 사용하고, 개발 서버의 API 및 WebSocket 프록시를 통해 Rust 백엔드(127.0.0.1:8080)와 통신합니다.

## 2. 주요 기능

- KOSPI200 정규선물·미니선물 현재가 및 실시간 체결
- 분봉 차트 및 분봉 주기 전환
- Supabase Google OAuth 로그인
- 분석 글 목록 및 Markdown 상세 보기
- 관리자 분석 글 작성·수정·삭제
- 관리자 뉴스 링크 등록·삭제

## 3. 프로젝트 구조

    backend/
      src/
        auth/       # JWT 검증
        cache/      # Redis 캐시
        chat/       # 익명 별칭, 연결 티켓 및 메시지 검증
        core/       # bootstrap, AppState, CandleStore, DB, cleanup
        kis/        # KIS 인증, 시세, 분봉 API
        kis_ws/     # KIS WebSocket 프로토콜 및 클라이언트
        market/     # 과거 시세 로딩 및 변환
        server/     # 라우터, 런타임, HTTP/WS 핸들러
      .env.example
      Cargo.toml

    config/
      nicknames.json  # 운영자가 관리하는 채팅 별칭 단어 및 번호 범위

    frontend/
      src/
        components/   # AppHeader, AppModal
        features/
          admin/      # 관리자 글/뉴스 관리
          analysis/   # 분석 글 목록/상세
          auth/       # 인증 세션 및 관리자 권한
          news/       # 뉴스 링크 표시
          prices/     # 시세 화면, WebSocket, 차트 데이터 훅
        pages/        # Analysis, Admin, Login
        App.tsx
        App.css
        CandleChart.tsx
        PriceCard.tsx
      .env.example
      package.json

    DESIGN.md
    Readme.md

## 4. 사전 준비

- Rust toolchain (프로젝트 기준 1.100.0 이상)
- Node.js 22.x 이상 및 npm
- Docker Desktop
- 한국투자증권 Open API 키
- Supabase 프로젝트 및 Google OAuth 설정

## 5. 환경 변수 설정

실제 비밀 키를 저장하는 .env 파일은 Git에 커밋하지 마세요.

### Backend

① Project main PowerShell에서 프로젝트 폴더에 있는지 확인한 뒤 실행합니다.

    cd backend
    Copy-Item .env.example .env

backend/.env에 KIS 앱 키·시크릿, 계좌 정보, Supabase URL 및 키, JWT Secret 등을 실제 환경에 맞게 입력합니다. 변수 이름과 설명은 backend/.env.example을 기준으로 확인하세요. Service Role Key와 JWT Secret은 백엔드 전용 비밀 값이며 프론트엔드에 넣지 않습니다.

채팅 별칭은 저장소 루트 `config/nicknames.json`에서 읽습니다. 명령 응답은 `config/chat_commands.json`에서 읽으며, 입력 전체가 명령 키와 일치하면 서버가 `{nickname}`을 연결의 별칭으로 치환해 전달합니다. 두 설정 모두 백엔드 시작 시 한 번 읽으므로 변경하려면 재시작/재배포가 필요하고, 설정이 없거나 유효하지 않으면 백엔드 시작이 실패합니다. 신뢰 proxy 뒤에서 운영할 때는 `TRUSTED_PROXY_IPS`를 설정하고 proxy가 `X-Forwarded-For`를 안전하게 덮어쓰도록 구성하세요.

### Frontend

① Project main PowerShell에서 실행합니다.

    cd ..\frontend
    Copy-Item .env.example .env

기본 개발 설정은 VITE_PORT=5173, VITE_DEV_BACKEND_URL=http://127.0.0.1:8080입니다. 실제 프론트엔드 환경 변수 목록은 frontend/.env.example을 기준으로 합니다.

## 6. 로컬 실행

각 프로세스는 별도의 PowerShell 창에서 실행합니다.

### 1) Redis

Docker Desktop을 실행한 후 Redis 컨테이너를 시작합니다. 최초 생성 시 한 번만 실행합니다.

    docker run -d --name stock-redis --restart unless-stopped -p 6379:6379 redis:7-alpine

이미 컨테이너가 생성되어 있다면 Docker Desktop에서 시작하거나 다음 명령을 사용합니다.

    docker start stock-redis
    docker exec -it stock-redis redis-cli ping

정상 응답은 PONG입니다.

### 2) Backend

② Backend PowerShell (backend 폴더)에서 실행합니다.

    cargo run

기본 로컬 주소는 http://127.0.0.1:8080입니다. 실제 바인딩 주소는 BACKEND_BIND_ADDR 설정을 따릅니다.

### 3) Frontend

③ Frontend PowerShell (frontend 폴더)에서 실행합니다.

    npm install
    npm run dev

브라우저에서 http://localhost:5173을 엽니다.

## 7. 검증 명령

명령은 해당 폴더에서 실행합니다.

② Backend PowerShell:

    cargo check
    cargo test

③ Frontend PowerShell:

    npm run lint
    npm run build

## 8. API 및 운영 참고

- 시세 API와 브라우저 WebSocket 경로는 백엔드 라우터에서 관리합니다.
- Phase 1에서는 비인증 `POST /api/chat/ticket`으로 임시 채팅 티켓/별칭을 발급합니다. WebSocket fan-out과 채팅 UI는 아직 구현되지 않았습니다.
- 분석 글·뉴스 API는 인증이 필요하며, 관리자 변경 API는 관리자 권한 검증을 거칩니다.
- 콘텐츠 보존·삭제 정책, 인증 및 데이터 모델의 상세 설계는 DESIGN.md를 참고하세요.
- 주말/휴장일의 최근 거래일 차트 표시 정책은 DESIGN.md에 미확정 안건으로 기록되어 있습니다. 합의 전에는 날짜 조회 정책을 임의로 변경하지 않습니다.

## 9. 보안 주의사항

- .env, .token, 서비스 키, JWT Secret, 계좌 정보는 저장소에 커밋하지 않습니다.
- Supabase Service Role Key는 백엔드에서만 사용합니다.
- 뉴스는 제목과 링크 중심으로 관리하며 기사 본문 전문을 무단 저장하지 않습니다.
