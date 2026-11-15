use tokio::sync::{broadcast, mpsc};

pub(super) const TR_ID: &str = "H0IFCNT0";

#[derive(Debug, Clone)]
pub struct RealtimeTick {
    pub code: String,
    pub date: String,
    pub price: String,
    pub change: String,
    pub change_rate: String,
    pub volume: String,
    pub last_qty: String,
    pub time: String,
}

pub(super) fn handle_text(
    text: &str,
    tx: &broadcast::Sender<RealtimeTick>,
    pong_tx: &mpsc::Sender<String>,
) {
    if text.contains("\"tr_id\":\"PINGPONG\"") {
        tracing::debug!("PINGPONG 수신, 응답 전송 예약");
        let _ = pong_tx.try_send(text.to_string());
        return;
    }

    let parts: Vec<&str> = text.split('|').collect();
    if parts.len() >= 4 && parts[1] == TR_ID {
        let fields: Vec<&str> = parts[3].split('^').collect();
        if fields.len() > 10 {
            let today = chrono::Local::now().format("%Y%m%d").to_string();
            let tick = RealtimeTick {
                code: fields[0].to_string(),
                date: today,
                time: fields[1].to_string(),
                change: format!("{}{}", fields[3], fields[2]),
                change_rate: fields[4].to_string(),
                price: fields[5].to_string(),
                last_qty: fields[9].to_string(),
                volume: fields[10].to_string(),
            };
            let _ = tx.send(tick);
        }
        return;
    }
    tracing::debug!("WS 응답: {}", text);
}
