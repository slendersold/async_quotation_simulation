//! Загрузка и валидация списка тикеров из файла.

use std::sync::OnceLock;

const TICKERS_RAW: &str = include_str!("../../tickers.txt");

static TICKERS: OnceLock<Vec<&'static str>> = OnceLock::new();

fn collect_tickers(tickers_raw: &'static str) -> Vec<&'static str> {
    tickers_raw
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect()
}

pub fn all(tickers_raw: &'static str) -> &'static [&'static str] {
    TICKERS.get_or_init(|| collect_tickers(tickers_raw)).as_slice()
}

/// Возвращает список тикеров из стандартного файла `tickers.txt`.
///
/// Список формируется один раз при первом вызове (lazy init через `OnceLock`).
pub fn all_default() -> &'static [&'static str] {
    all(TICKERS_RAW)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_tickers_trims_and_skips_empty_lines() {
        assert_eq!(collect_tickers(" A \n\nB "), vec!["A", "B"]);
    }

    #[test]
    fn all_default_is_non_empty_and_starts_with_aapl() {
        let list = all_default();
        assert!(!list.is_empty());
        assert_eq!(list[0], "AAPL");
    }
}
