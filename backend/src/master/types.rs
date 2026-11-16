#[derive(Debug, Clone)]
pub struct FuturesSymbol {
    pub info_type: String,
    pub code: String,
    pub name: String,
    pub month_ym: (i32, u32),
}
