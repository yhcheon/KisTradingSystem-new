// ============================================================
// 파일명: core/cleanup.rs
// 설명: Supabase 오래된 데이터 정리 스케줄러
//       - 매일 KST 07:00에 30일 지난 데이터 삭제
// ============================================================

use crate::core::db::SupabaseClient;

/// 매일 KST 07:00에 Supabase의 오래된 데이터를 정리하는 백그라운드 작업을 시작합니다.
pub fn spawn_daily_cleanup(supabase: SupabaseClient) {
    tokio::spawn(async move {
        use chrono::TimeZone;
        let kst = chrono_tz::Asia::Seoul;
        loop {
            let now = chrono::Utc::now().with_timezone(&kst);
            let Some(today_7am) = now.date_naive().and_hms_opt(7, 0, 0) else {
                tracing::error!("KST 데이터 정리 기준 시각 계산 실패");
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                continue;
            };
            let Some(seven_am) = chrono::NaiveTime::from_hms_opt(7, 0, 0) else {
                tracing::error!("KST 데이터 정리 시각 생성 실패");
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                continue;
            };
            let target_local = if now.time() < seven_am {
                Some(today_7am)
            } else {
                today_7am.checked_add_signed(chrono::Duration::days(1))
            };
            let Some(target_local) = target_local else {
                tracing::error!("다음 데이터 정리 날짜 계산 실패");
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                continue;
            };
            let Some(next_7am) = kst.from_local_datetime(&target_local).single() else {
                tracing::error!("다음 KST 데이터 정리 시각 변환 실패");
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                continue;
            };
            let wait = (next_7am - now)
                .to_std()
                .unwrap_or(std::time::Duration::from_secs(60));

            tracing::info!(
                "다음 데이터 삭제 예정: {} ({}초 후)",
                next_7am,
                wait.as_secs()
            );
            tokio::time::sleep(wait).await;

            if let Err(e) = supabase.cleanup_old_data().await {
                tracing::warn!("오래된 데이터 삭제 실패: {}", e);
            } else {
                tracing::info!("30일 지난 데이터 삭제 완료");
            }
        }
    });
}
