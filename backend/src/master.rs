// ============================================================
// 파일명: master.rs
// 설명: KIS 종목 마스터 파일 다운로드 및 파싱
// 작성일: 2026-11-14
// ============================================================

use std::io::Read;

#[derive(Debug, Clone)]
pub struct FuturesSymbol {
    pub info_type: String,
    pub code: String,
    pub name: String,
    pub month_code: String,
    pub month_ym: (i32, u32),
}

pub async fn download_master_text(
    client: &reqwest::Client,
) -> Result<String, Box<dyn std::error::Error>> {
    let url = "https://new.real.download.dws.co.kr/common/master/fo_idx_code_mts.mst.zip";
    let bytes = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;

    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor)?;

    let mut file = archive.by_index(0)?;
    let mut raw = Vec::new();
    file.read_to_end(&mut raw)?;

    let (decoded, _, _) = encoding_rs::EUC_KR.decode(&raw);
    Ok(decoded.into_owned())
}

pub fn parse_master(text: &str) -> Vec<FuturesSymbol> {
    let mut result = Vec::new();
    for line in text.lines() {
        let fields: Vec<&str> = line.split('|').collect();
        if fields.len() < 4 {
            continue;
        }
        let info_type = fields[0].trim();
        if info_type != "1" && info_type != "B" {
            continue;
        }
        let code = fields[1].trim().to_string();
        let name = fields[3].trim().to_string();
        let month_code = name.split_whitespace().last().unwrap_or("").to_string();
        let month_ym = if month_code.len() == 6 {
            (
                month_code[0..4].parse::<i32>().unwrap_or(0),
                month_code[4..6].parse::<u32>().unwrap_or(0),
            )
        } else {
            (0, 0)
        };
        result.push(FuturesSymbol {
            info_type: info_type.to_string(),
            code,
            name,
            month_code,
            month_ym,
        });
    }
    result
}

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
