//! Модель котировки и (де)сериализация: структуры данных и форматы передачи.

#[derive(Debug, Clone, PartialEq)]
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
