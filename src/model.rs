//! Модель котировки и форматы сериализации.
//!
//! Датаграммы UDP с котировкой — одна строка JSON:
//! `{"ticker":"AAPL","price":...,"volume":...,"timestamp":...}`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StockQuote {
    pub ticker: String,
    pub price: f64,
    pub volume: u32,
    pub timestamp: u64,
}

impl StockQuote {
    pub fn to_string(&self) -> String {
        format!(
            "{}|{}|{}|{}",
            self.ticker, self.price, self.volume, self.timestamp
        )
    }

    pub fn from_string(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('|').collect();
        if parts.len() != 4 {
            return None;
        }

        Some(StockQuote {
            ticker: parts[0].to_string(),
            price: parts[1].parse().ok()?,
            volume: parts[2].parse().ok()?,
            timestamp: parts[3].parse().ok()?,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.to_string().into_bytes()
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let s = std::str::from_utf8(bytes).ok()?;
        Self::from_string(s)
    }

    /// Сериализация котировки в одну строку JSON для UDP.
    pub fn to_json_line(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }

    /// Разбор JSON-строки котировки из UDP.
    pub fn from_json_line(s: &str) -> Option<Self> {
        serde_json::from_str(s.trim()).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::generator::QuoteGenerator;
    use std::time::Duration;

    static DEMO_TICKERS: &[&str] = &["AAPL", "ZZZ"];

    /// Эталонная UTF-8 линия `ticker|price|volume|timestamp` для сравнения с [`StockQuote::to_bytes`].
    fn wire_line_bytes_via_format(q: &StockQuote) -> Vec<u8> {
        format!("{}|{}|{}|{}", q.ticker, q.price, q.volume, q.timestamp).into_bytes()
    }

    #[test]
    fn roundtrip_to_string_from_string() {
        let q = StockQuote {
            ticker: "TEST".to_string(),
            price: 123.45,
            volume: 999,
            timestamp: 1_700_000_000_000,
        };
        let s = q.to_string();
        let back = StockQuote::from_string(&s).expect("parse");
        assert_eq!(back, q);
    }

    #[test]
    fn from_string_rejects_wrong_field_count() {
        assert!(StockQuote::from_string("a|b|c").is_none());
    }

    #[test]
    fn roundtrip_bytes() {
        let q = StockQuote {
            ticker: "X".to_string(),
            price: 1.0,
            volume: 2,
            timestamp: 3,
        };
        let bytes = q.to_bytes();
        let back = StockQuote::from_bytes(&bytes).expect("parse bytes");
        assert_eq!(back, q);
    }

    #[test]
    fn serialization_to_bytes_matches_string_line() {
        let samples = [
            StockQuote {
                ticker: "AAPL".into(),
                price: 123.45,
                volume: 999,
                timestamp: 1_700_000_000_000,
            },
            StockQuote {
                ticker: "X".into(),
                price: 1.0,
                volume: 2,
                timestamp: 3,
            },
            StockQuote {
                ticker: "ММК".into(),
                price: 50.0,
                volume: 1,
                timestamp: 0,
            },
            StockQuote {
                ticker: "WEIRD|SYM".into(),
                price: f64::EPSILON,
                volume: u32::MAX,
                timestamp: u64::MAX,
            },
        ];
        for q in samples {
            assert_eq!(
                q.to_bytes(),
                wire_line_bytes_via_format(&q),
                "to_bytes vs format! line"
            );
            assert_eq!(
                q.to_string(),
                String::from_utf8(q.to_bytes()).unwrap(),
                "to_string vs utf8(to_bytes)"
            );
        }
    }

    #[test]
    fn serialization_generator_quotes_match_string_line() {
        let mut generator =
            QuoteGenerator::new_with_seed_interval_for(DEMO_TICKERS, 0xC0FFEE, Duration::ZERO);
        for _ in 0..5 {
            generator.advance_batch();
            for ticker in ["AAPL", "ZZZ"] {
                let q = generator.last_batch_quote(ticker).expect("ticker in demo list");
                assert_eq!(q.to_bytes(), wire_line_bytes_via_format(&q));
                assert_eq!(
                    q.to_string(),
                    String::from_utf8(wire_line_bytes_via_format(&q)).unwrap()
                );
            }
        }
    }

    #[test]
    fn serialization_from_bytes_roundtrip_wire_line() {
        let q = StockQuote {
            ticker: "TSLA".into(),
            price: 200.125,
            volume: 42,
            timestamp: 99,
        };
        let line = wire_line_bytes_via_format(&q);
        assert_eq!(StockQuote::from_bytes(&line), Some(q.clone()));
        assert_eq!(StockQuote::from_string(&String::from_utf8(line).unwrap()), Some(q));
    }

    #[test]
    fn json_line_roundtrip_includes_expected_fields() {
        let q = StockQuote {
            ticker: "AAPL".into(),
            price: 150.5,
            volume: 1000,
            timestamp: 1_700_000_000_000,
        };
        let j = q.to_json_line().expect("json");
        assert!(j.contains("\"ticker\":\"AAPL\""));
        assert!(j.contains("\"price\"") && j.contains("\"volume\"") && j.contains("\"timestamp\""));
        assert_eq!(StockQuote::from_json_line(&j), Some(q));
    }
}
