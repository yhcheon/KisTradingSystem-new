use super::types::FuturesSymbol;

pub fn pick_nearest(symbols: &[FuturesSymbol], info_type: &str) -> Option<FuturesSymbol> {
    let now = chrono::Local::now();
    let current_ym = (
        now.format("%Y").to_string().parse::<i32>().unwrap_or(0),
        now.format("%m").to_string().parse::<u32>().unwrap_or(0),
    );
    symbols
        .iter()
        .filter(|s| s.info_type == info_type && s.month_ym.0 > 0)
        .filter(|s| s.month_ym >= current_ym)
        .min_by_key(|s| s.month_ym)
        .cloned()
}
