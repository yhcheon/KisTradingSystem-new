// ============================================================
// 파일명: core/cleanup.rs  (PATCHED)
// 변경점: spawn_daily_cleanup(supabase, shutdown) 로 시그니처 변경.
//         다음 07:00까지 sleep하는 동안 종료 신호가 오면 즉시 리턴.
// ============================================================

use crate::core::db::SupabaseClient;
use std::sync::Arc;
use tokio::sync::Notify;
use tokio::task::JoinHandle;





pub fn spawn_daily_cleanup(supabase: SupabaseClient, shutdown: Arc<Notify>) -> JoinHandle<()> {
    tokio::spawn(async move {
        use chrono::TimeZone;
        let kst = chrono_tz::Asia::Seoul;
        loop {
            let now = chrono::Utc::now().with_timezone(&kst);
            let Some(today_7am) = now.date_naive().and_hms_opt(7, 0, 0) else {
                tracing::error!("KST 데이터 정리 기준 시각 계산 실패");
                if shutdown_wait_or_timeout(shutdown.clone(), 60).await { break; }
                continue;
            };
            let Some(seven_am) = chrono::NaiveTime::from_hms_opt(7, 0, 0) else {
                tracing::error!("KST 데이터 정리 시각 생성 실패");
                if shutdown_wait_or_timeout(shutdown.clone(), 60).await { break; }
                continue;
            };
            let target_local = if now.time() < seven_am { Some(today_7am) }
                else { today_7am.checked_add_signed(chrono::Duration::days(1)) };
            let Some(target_local) = target_local else {
                tracing::error!("다음 데이터 정리 날짜 계산 실패");
                if shutdown_wait_or_timeout(shutdown.clone(), 60).await { break; }
                continue;
            };
            let Some(next_7am) = kst.from_local_datetime(&target_local).single() else {
                tracing::error!("다음 KST 데이터 정리 시각 변환 실패");
                if shutdown_wait_or_timeout(shutdown.clone(), 60).await { break; }
                continue;
            };
            let wait = (next_7am - now).to_std().unwrap_or(std::time::Duration::from_secs(60));
            tracing::info!("다음 데이터 삭제 예정: {} ({}초 후)", next_7am, wait.as_secs());
            tokio::select! {
                _ = shutdown.notified() => {
                    tracing::info!("종료 신호 수신 - daily cleanup 태스크 종료");
                    break;
                }
                _ = tokio::time::sleep(wait) => {}
            }
            if let Err(e) = supabase.cleanup_old_data().await {
                tracing::warn!("오래된 데이터 삭제 실패: {}", e);
            } else {
                tracing::info!("30일 지난 데이터 삭제 완료");
            }
        }
        tracing::info!("daily cleanup 태스크 종료 완료");
    })
}




async fn shutdown_wait_or_timeout(shutdown: Arc<Notify>, secs: u64) -> bool {
    tokio::select! {
        _ = shutdown.notified() => true,
        _ = tokio::time::sleep(std::time::Duration::from_secs(secs)) => false,
    }
}



