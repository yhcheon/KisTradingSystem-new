
# KOSPI200 선물 시세 웹서비스 — 전체 설계 문서

> 최종 업데이트: 2026-11-14
> 대상: 개발자, 운영자

---

## 1. 프로젝트 개요

| 항목 | 내용 |
|---|---|
| **목표** | KOSPI200 정규선물/미니선물 시세 조회 + 양질의 분석 콘텐츠 제공 |
| **사용자** | 약 200명 (내부/제한 사용자) |
| **데이터 소스** | 한국투자증권 KIS Open API (REST + WebSocket) |
| **아키텍처** | Rust 백엔드 + React 프론트엔드 + Supabase(인증/콘텐츠) |

---

## 2. 전체 시스템 구성

[사용자 브라우저]
│
├── (1) 시세 데이터 ──→ [Rust 백엔드] ──→ [KIS API]
│ │
│ └── (캐시) Redis
│
├── (2) 로그인 ──────→ [Supabase Auth] (구글 OAuth)
│
└── (3) 분석 글/뉴스 ─→ [Supabase PostgreSQL]
│
└── (30일 자동 삭제) pg_cron




### 역할 분담

| 계층 | 담당 | 저장소 |
|---|---|---|
| **시세/차트** | Rust 백엔드 (Axum + Tokio) | 메모리(CandleStore) + Redis 캐시 |
| **로그인** | Supabase Auth (구글 OAuth) | Supabase |
| **관리자 글/뉴스** | Rust 백엔드 API → Supabase DB | Supabase PostgreSQL |
| **프론트엔드** | React + TypeScript + Vite | - |

---

## 3. 기술 스택

| 계층 | 기술 | 버전 |
|---|---|---|
| 백엔드 언어 | Rust | 1.100.0 |
| 백엔드 프레임워크 | Axum | 0.8 |
| 비동기 런타임 | Tokio | 1.x |
| HTTP 클라이언트 | reqwest (연결 풀 공유) | 0.12 |
| 캐시 | Redis | 7-alpine (Docker) |
| 인증 | Supabase Auth (구글 OAuth) | - |
| 콘텐츠 DB | Supabase PostgreSQL | - |
| 프론트엔드 | React + TypeScript + Vite | - |
| 차트 | TradingView Lightweight Charts | - |
| Markdown | react-markdown | - |
| 로깅 | tracing + tracing-appender | 0.3 / 0.2 |

---

## 4. 데이터 모델 (Supabase)

### 4.1 `notes` — 관리자 분석 글

```sql
CREATE TABLE notes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    category TEXT NOT NULL,          -- 'us_market' | 'kospi_plan'
    title TEXT NOT NULL,
    content TEXT NOT NULL,           -- Markdown 본문
    author_email TEXT NOT NULL,      -- 작성자 (관리자)
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_notes_created_at ON notes (created_at DESC);
CREATE INDEX idx_notes_category ON notes (category);



CREATE TABLE news_links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title TEXT NOT NULL,
    url TEXT NOT NULL,
    source TEXT,                     -- '한경' | '매경' | ...
    published_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_news_created_at ON news_links (created_at DESC);





CREATE TABLE user_profiles (
    user_id UUID PRIMARY KEY REFERENCES auth.users(id) ON DELETE CASCADE,
    tier TEXT NOT NULL DEFAULT 'free',   -- 'free' | 'paid'
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);






[React] supabase.auth.signInWithOAuth({ provider: 'google' })
      ↓
[Supabase Auth] 구글 OAuth 인증 → JWT 발급
      ↓
[React] 세션 저장 (localStorage or cookie)
      ↓
[React] API 호출 시 Authorization: Bearer <JWT>
      ↓
[Rust 백엔드] supabase-jwt로 JWT 검증 → 사용자 ID 추출




5.2 무료 회원 시간 제한
등급	조건	API 응답
paid	언제나	✅ 200 OK
free	서버 시각 KST 14:00 이후	✅ 200 OK
free	서버 시각 KST 14:00 이전	❌ 403 Forbidden
핵심: 판단은 반드시 백엔드에서 (프론트엔드 시계 조작 방지).


// 의사 코드
let user = verify_supabase_jwt(&token)?;
let profile = get_user_profile(&supabase, &user.id).await?;
if profile.tier == "free" && chrono::Local::now().hour() < 14 {
    return Err(StatusCode::FORBIDDEN, "무료 회원은 오후 2시부터 열람 가능합니다");
}









5.3 관리자 권한
관리자 판별: user_profiles.tier = 'admin' (또는 이메일 화이트리스트)

관리자만 POST /api/notes, POST /api/news 호출 가능







6.1 공개 API (누구나)
메서드	경로	설명
GET	/api/health	서버 상태
GET	/api/prices	정규/미니선물 현재가 (Redis 캐시 TTL 3초)
GET	/api/candles?code=&interval=&date=	분봉 조회 (메모리)
WS	/ws	실시간 체결



6.2 인증 필요 API
메서드	경로	권한	설명
GET	/api/notes?date=	로그인 (free는 14시 이후)	관리자 글 목록
GET	/api/notes/:id	로그인 (free는 14시 이후)	글 상세
GET	/api/news?limit=	로그인 (free는 14시 이후)	뉴스 링크 목록


6.3 관리자 API
메서드	경로	권한	설명
POST	/api/admin/notes	admin	글 작성
PUT	/api/admin/notes/:id	admin	글 수정
DELETE	/api/admin/notes/:id	admin	글 삭제
POST	/api/admin/news	admin	뉴스 링크 추가
DELETE	/api/admin/news/:id	admin	뉴스 링크 삭제





7. 프론트엔드 화면 구성
7.1 탭 구조


┌─────────────────────────────────────────────────┐
│ KOSPI200 선물 시세       [시세] [분석] [관리자]  │
├─────────────────────────────────────────────────┤
│                                                 │
│ [시세 탭]                                       │
│  ┌────────┐  ┌──────────────────────────────┐  │
│  │정규선물│  │                              │  │
│  │미니선물│  │      캔들 차트 (분봉)        │  │
│  └────────┘  │                              │  │
│              └──────────────────────────────┘  │
│                                                 │
│ [분석 탭]                                       │
│  ┌───────────────────────┐  ┌────────────────┐ │
│  │ 글 목록               │  │ 선택된 글 본문 │ │
│  │ - 미국장 분석 11/14   │  │ (Markdown)     │ │
│  │ - 코스피 대응 11/14   │  │                │ │
│  └───────────────────────┘  └────────────────┘ │
│                                                 │
│ [관리자 탭] (admin만 보임)                      │
│  - 글 작성 폼 (제목 + Markdown 에디터)          │
│  - 뉴스 링크 추가 폼                            │
└─────────────────────────────────────────────────┘


7.2 화면별 동작
화면	등급	동작
로그인	비로그인	"구글 로그인" 버튼 → Supabase OAuth
시세 탭	누구나	현재가 카드 + 차트
분석 탭 (paid)	로그인	글 목록 + 본문
분석 탭 (free, 14시 이후)	로그인	글 목록 + 본문
분석 탭 (free, 14시 이전)	로그인	"오후 2시부터 열람 가능합니다"
관리자 탭	admin	글 작성 + 뉴스 추가



8. 구현 단계 (로드맵)
Phase 1: Supabase 셋업 (1일)
□ Supabase 프로젝트 생성
□ Google Cloud Console에서 OAuth Client ID/Secret 발급
□ Supabase Auth → Google 프로바이더 활성화
□ 테이블 생성 (notes, news_links, user_profiles)
□ pg_cron 확장 + 자동 삭제 잡 등록
Phase 2: 백엔드 인증 (2일)
□ supabase-jwt 크레이트 추가
□ JWT 검증 미들웨어 (Axum)
□ user_profiles 조회 + 등급 확인
□ 무료 회원 14시 제한 로직
Phase 3: 백엔드 콘텐츠 API (2일)
□ Supabase PostgREST 클라이언트 (또는 reqwest로 직접 호출)
□ /api/notes (GET, POST)
□ /api/news (GET, POST)
□ 관리자 권한 체크
Phase 4: 프론트엔드 (3일)
□ @supabase/supabase-js 설치
□ 로그인 페이지 (구글 OAuth)
□ 탭 구조 ([시세] [분석] [관리자])
□ 분석 탭 (글 목록 + Markdown 렌더링)
□ 관리자 탭 (글 작성 + 뉴스 추가)
□ 무료 회원 14시 제한 UI (에러 메시지)
Phase 5: 테스트 및 배포 (2일)
□ 로컬 통합 테스트
□ AWS/VPS 배포 (Docker)
□ 모니터링 설정
9. 주의 사항 및 제약
9.1 Supabase 무료 플랜
항목	제한	대응
DB 용량	500 MB	관리자 글 + 뉴스 링크엔 충분
비활성 일시정지	7일	접속/크론으로 방지
MAU	50,000	200명 규모엔 무관
9.2 시간 제한
KST 기준으로 판단 (서버가 UTC여도 chrono::Local을 KST로 변환)

프론트엔드 시계 조작 방지를 위해 백엔드에서만 판단

9.3 보안
Supabase JWT는 백엔드에서 반드시 검증 (프론트엔드 신뢰 금지)

관리자 API는 tier = 'admin' 확인

.env의 Supabase 키는 절대 Git 커밋 금지

9.4 저작권 (뉴스)
뉴스 제목 + 링크만 저장 (본문 전문 스크랩 금지)

출처 명시









### 4.4 30일 지난 데이터 삭제 (매일 KST 07:00)

서버가 켜져 있는 동안 **매일 아침 7시(KST)** 에 `notes`와 `news_links`에서 생성 후 30일이 지난 항목을 삭제합니다. 관리자 분석 글도 예외 없이 동일한 보존 기간을 적용합니다.
- 07:00은 주간장(08:45) 시작 전, 야간장(06:00) 종료 후 → **장 운영에 영향 없음**
- 서버가 7시에 꺼져 있으면 그날은 건너뛰고, 다음날 7시에 실행 (며칠 쌓여도 무방)
- Supabase "7일 비활성 일시정지" 방지 효과도 겸함

**구현**: `main.rs`에서 `tokio::spawn`으로 백그라운드 루프 실행.

```rust
// main.rs (Supabase 클라이언트 초기화 후)
let supabase_for_cleanup = supabase.clone();
tokio::spawn(async move {
    loop {
        // 다음 KST 07:00까지 남은 시간 계산
        let now = chrono::Local::now();
        let today_7am = now.date_naive()
            .and_hms_opt(7, 0, 0)
            .expect("invalid time")
            .and_local_timezone(chrono::Local)
            .single()
            .expect("ambiguous time");
        let next_7am = if now < today_7am {
            today_7am
        } else {
            today_7am + chrono::Duration::days(1)
        };
        let wait = (next_7am - now)
            .to_std()
            .unwrap_or(std::time::Duration::from_secs(60));

        tracing::info!("다음 데이터 삭제 예정: {} ({}초 후)", next_7am, wait.as_secs());
        tokio::time::sleep(wait).await;

        // 30일 지난 데이터 삭제 실행
        if let Err(e) = cleanup_old_data(&supabase_for_cleanup).await {
            tracing::warn!("오래된 데이터 삭제 실패: {}", e);
        } else {
            tracing::info!("30일 지난 데이터 삭제 완료");
        }
    }
});

---

## 10. 운영 및 차트 데이터 기준

### 10.1 Redis 선행 실행

백엔드는 시작 과정에서 `127.0.0.1:6379`의 Redis에 연결하고 PING을 수행합니다. 따라서 백엔드보다 Docker Desktop의 Redis 컨테이너가 먼저 실행되어 있어야 합니다. 실행 방법은 `Readme.md`의 **실행 방법 → Redis 실행**을 따릅니다.

### 10.2 최근 거래일 차트 표시 정책(회의 안건, 미확정)

주말에도 차트에 최근 거래일 데이터가 표시되어야 한다는 운영 요구가 확인되었습니다. 현재 확인된 기대 동작은 다음과 같습니다.

| 조회 기준일 | 차트에 표시할 최근 2개 거래일 |
|---|---|
| 금요일 | 금요일 + 목요일 |
| 토요일 | 금요일 + 목요일 |
| 일요일 | 금요일 + 목요일 |

핵심은 달력 날짜의 어제/오늘이 아니라 **실제 거래 데이터가 존재하는 최근 거래일 2개**를 기준으로 조회하는 것입니다.

현재 구현에는 다음과 같은 날짜 기준 불일치가 있습니다.
- 초기 데이터 로더(`market/history.rs`)는 최근 과거 날짜를 조회해 직전 거래일을 찾지만, 기본적으로 직전 거래일 데이터만 로드합니다. 당일 데이터는 평일 장중 조건에서만 로드합니다.
- 차트 API(`server/handlers/market.rs`)의 기본 `date=both` 처리는 달력 기준 어제와 오늘을 사용합니다. 주말에는 토요일/일요일을 조회하게 되어 금요일/목요일 데이터가 표시되지 않을 수 있습니다.

**회의에서 결정할 사항**
1. 초기 구동 시 최근 거래일 데이터를 정확히 2일치 가져올 것인지, 아니면 최근 데이터가 존재하는 날짜부터 과거 방향으로 2개 거래일을 채울 것인지.
2. KIS 분봉 API의 조회 결과에서 실제 거래일을 판별하고, 휴장일/주말을 건너뛰는 책임을 백엔드 초기 로더와 조회 API 중 어디에 둘 것인지.
3. 장중에는 최근 완료된 거래일 1일과 당일 진행 중 데이터(최대 2개 날짜)를 보여줄지, 항상 거래일 2일 + 당일 진행 데이터를 보여줄지.
4. 데이터가 없는 종목/날짜가 있을 때 두 종목의 표시 날짜를 공통으로 맞출지, 종목별로 독립 판별할지.

위 항목은 **요구사항 및 회의 안건으로 기록한 것이며, 구현 정책은 아직 확정되지 않았습니다.** 합의 전에는 날짜 계산이나 초기 데이터 적재 로직을 변경하지 않습니다.

