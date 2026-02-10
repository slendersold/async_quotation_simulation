//! Загрузка и валидация списка тикеров из файла.

use std::sync::OnceLock;

// const TICKERS_RAW: &str = include_str!("../../tickers.txt");

static TICKERS: OnceLock<Vec<&'static str>> = OnceLock::new();

pub fn all(tickers_raw: &'static str) -> &'static [&'static str] {
    TICKERS
        .get_or_init(|| {
            tickers_raw
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .collect()
        })
        .as_slice()
}
