//! Загрузка и валидация списка тикеров из файла.

use std::io::{self, BufRead, BufReader};
use std::path::Path;
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

/// Тикеры из файла: по одному в строке, пустые строки пропускаются.
pub fn load_tickers_from_path(path: &Path) -> io::Result<Vec<String>> {
    let f = std::fs::File::open(path)?;
    let mut out = Vec::new();
    for line in BufReader::new(f).lines() {
        let s = line?.trim().to_string();
        if !s.is_empty() {
            out.push(s);
        }
    }
    if out.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "tickers file is empty or has no non-empty lines",
        ));
    }
    Ok(out)
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

    #[test]
    fn load_tickers_from_path_reads_tickers_txt() {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tickers.txt");
        let list = load_tickers_from_path(&p).unwrap();
        assert!(list.contains(&"AAPL".to_string()));
        assert!(list.contains(&"TSLA".to_string()));
    }
}
