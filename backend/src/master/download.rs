use std::io::Read;

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
