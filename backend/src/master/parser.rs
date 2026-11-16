use super::types::FuturesSymbol;

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
        let month_code = name.split_whitespace().last().unwrap_or("");
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
            month_ym,
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_year_month_from_symbol_name() {
        let symbols = parse_master("1|101S|unused|KOSPI200 202612");

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].month_ym, (2026, 12));
    }
}
