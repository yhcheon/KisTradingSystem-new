# Frontend

이 디렉터리는 KOSPI200 선물 시세 웹서비스의 React + TypeScript + Vite 프론트엔드입니다.

## 실행

프로젝트 루트의 Readme.md에서 환경 변수 설정 및 전체 서비스 실행 방법을 확인하세요.

Frontend PowerShell에서:

    npm install
    npm run dev

개발 서버는 기본적으로 http://localhost:5173에서 실행됩니다. 백엔드 프록시 주소는 .env의 VITE_DEV_BACKEND_URL 설정을 사용합니다.

## 검증

    npm run lint
    npm run build

## 주요 디렉터리

- src/components: 앱 헤더 및 모달
- src/features/admin: 관리자 글·뉴스 관리
- src/features/analysis: 분석 글 목록·상세
- src/features/auth: 인증 세션 및 권한
- src/features/news: 뉴스 링크 표시
- src/features/prices: 시세 화면, WebSocket 및 차트 데이터 훅
- src/pages: 분석·관리자·로그인 페이지
