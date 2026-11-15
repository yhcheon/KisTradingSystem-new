/// "HHMMSS"에서 1분 전 계산
pub(super) fn subtract_one_minute(hhmmss: &str) -> String {
    if hhmmss.len() < 6 {
        return "154500".to_string();
    }
    let h: u32 = hhmmss[0..2].parse().unwrap_or(15);
    let m: u32 = hhmmss[2..4].parse().unwrap_or(45);
    let s: u32 = hhmmss[4..6].parse().unwrap_or(0);
    let total = h * 3600 + m * 60 + s;
    let new_total = if total >= 60 { total - 60 } else { 0 };
    format!(
        "{:02}{:02}{:02}",
        new_total / 3600,
        (new_total % 3600) / 60,
        new_total % 60
    )
}
