//! Генератор котировок: состояние по тикерам и **пакетная выдача** с одним временем.
//!
//! Каждый «тик» выпуска:
//! 1. Первый вызов [`QuoteGenerator::advance_batch`] фиксирует стартовые `price`/`volume` для
//!    всех тикеров и присваивает им **одинаковый** `timestamp`.
//! 2. Дальше: пересчитываются значения для **всех** тикеров, затем при необходимости
//!    ожидается остаток до [`QuoteGenerator::emit_interval`], после чего снова выставляется
//!    единый `timestamp` на весь пакет.
//!
//! Интервал между выпусками задаётся при создании генератора (по умолчанию 1 ms).

use crate::model::StockQuote;
use super::tickers;
use std::collections::HashMap;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// --- Параметры псевдо-ГПСЧ (xorshift64*) ---
const XORSHIFT_ZERO_SEED_FALLBACK: u64 = 0x9E3779B97F4A7C15;
const XORSHIFT_SHIFT_1: u32 = 12;
const XORSHIFT_SHIFT_2: u32 = 25;
const XORSHIFT_SHIFT_3: u32 = 27;
const XORSHIFT_MUL: u64 = 0x2545F4914F6CDD1D;

// --- Параметры бизнес-симуляции котировок ---
const SEED_MIX: u64 = 0xD1B54A32D192ED03;

const DEFAULT_EMIT_INTERVAL: Duration = Duration::from_millis(1);

const PRICE_START_MIN: f64 = 50.0;
const PRICE_START_SPAN: f64 = 950.0;

const VOLUME_START_MIN: u32 = 1_000;
const VOLUME_START_MOD: u64 = 10_000;

const DELTA_PRICE_BPS: i32 = 200;
const DELTA_PRICE_BPS_DENOM: f64 = 10_000.0;

const VOLUME_DRIFT_DIVISOR: u32 = 20;

const POPULAR_TICKERS: [&str; 3] = ["AAPL", "MSFT", "TSLA"];
const POPULAR_VOLUME_BASE: u32 = 1_000;
const POPULAR_VOLUME_EXTRA: u32 = 5_000;
const DEFAULT_VOLUME_BASE: u32 = 100;
const DEFAULT_VOLUME_EXTRA: u32 = 1_000;

#[derive(Debug, Clone)]
struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    fn new(seed: u64) -> Self {
        let seed = if seed == 0 {
            XORSHIFT_ZERO_SEED_FALLBACK
        } else {
            seed
        };
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> XORSHIFT_SHIFT_1;
        x ^= x << XORSHIFT_SHIFT_2;
        x ^= x >> XORSHIFT_SHIFT_3;
        self.state = x;
        x.wrapping_mul(XORSHIFT_MUL)
    }

    fn next_f64_01(&mut self) -> f64 {
        const DEN: f64 = (u64::MAX as f64) + 1.0;
        (self.next_u64() as f64) / DEN
    }

    fn next_i32_range(&mut self, min: i32, max: i32) -> i32 {
        debug_assert!(min <= max);
        if min == max {
            return min;
        }
        let span = (max - min + 1) as u32;
        let v = (self.next_u64() % span as u64) as i32;
        min + v
    }
}

fn now_millis_since_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn now_millis() -> u64 {
    now_millis_since_epoch()
}

/// Генератор котировок: пакетная выдача с единым временем на все тикеры в выпуске.
pub struct QuoteGenerator {
    emit_interval: Duration,
    rng: XorShift64,
    /// Порядок обхода тикеров (как в списке при создании) — детерминизм при одном seed.
    ticker_order: Vec<String>,
    last_price: HashMap<String, f64>,
    last_volume: HashMap<String, u32>,
    current_batch: HashMap<String, StockQuote>,
    /// Момент завершения предыдущего выпуска (`None` до первого `advance_batch`).
    last_emit_end: Option<Instant>,
}

impl QuoteGenerator {
    /// Генератор по `tickers.txt`, случайный seed, интервал выпуска **1 ms**.
    pub fn new() -> Self {
        Self::new_for(tickers::all_default())
    }

    /// Как [`Self::new`], но с заданным интервалом между выпусками.
    pub fn new_with_emit_interval(emit_interval: Duration) -> Self {
        let seed = now_millis() ^ SEED_MIX;
        Self::new_with_seed_interval_for(tickers::all_default(), seed, emit_interval)
    }

    pub fn new_for(tickers_list: &'static [&'static str]) -> Self {
        let seed = now_millis() ^ SEED_MIX;
        Self::new_with_seed_interval_for(tickers_list, seed, DEFAULT_EMIT_INTERVAL)
    }

    pub fn new_with_seed(seed: u64) -> Self {
        Self::new_with_seed_interval_for(tickers::all_default(), seed, DEFAULT_EMIT_INTERVAL)
    }

    pub fn new_with_seed_and_interval(seed: u64, emit_interval: Duration) -> Self {
        Self::new_with_seed_interval_for(tickers::all_default(), seed, emit_interval)
    }

    pub(crate) fn new_with_seed_interval_for(
        tickers_list: &'static [&'static str],
        seed: u64,
        emit_interval: Duration,
    ) -> Self {
        let mut rng = XorShift64::new(seed);
        let mut ticker_order = Vec::with_capacity(tickers_list.len());
        let mut last_price = HashMap::with_capacity(tickers_list.len());
        let mut last_volume = HashMap::with_capacity(tickers_list.len());

        for &t in tickers_list {
            let start_price = PRICE_START_MIN + rng.next_f64_01() * PRICE_START_SPAN;
            let start_volume = VOLUME_START_MIN + (rng.next_u64() % VOLUME_START_MOD) as u32;
            ticker_order.push(t.to_string());
            last_price.insert(t.to_string(), start_price);
            last_volume.insert(t.to_string(), start_volume);
        }

        Self {
            emit_interval,
            rng,
            ticker_order,
            last_price,
            last_volume,
            current_batch: HashMap::new(),
            last_emit_end: None,
        }
    }

    /// Интервал между завершением одного выпуска и следующим (после регенерации и ожидания).
    pub fn emit_interval(&self) -> Duration {
        self.emit_interval
    }

    pub fn set_emit_interval(&mut self, interval: Duration) {
        self.emit_interval = interval;
    }

    /// Выпускает следующий пакет: возвращает **единый** `timestamp` миллисекунд с Unix epoch
    /// для всех котировок этого пакета.
    ///
    /// - Первый вызов: без шага блуждания, только снимок стартовых значений + время.
    /// - Последующие: шаг для всех тикеров → при необходимости `sleep` до дедлайна
    ///   `предыдущий_конец + emit_interval` → снимок с новым общим временем.
    pub fn advance_batch(&mut self) -> u64 {
        if self.last_emit_end.is_none() {
            let ts = now_millis_since_epoch();
            self.rebuild_batch(ts);
            self.last_emit_end = Some(Instant::now());
            return ts;
        }

        for ticker in self.ticker_order.clone() {
            self.step_ticker(&ticker);
        }
        let after_regen = Instant::now();

        let deadline = self.last_emit_end.unwrap() + self.emit_interval;
        if let Some(wait) = deadline.checked_duration_since(after_regen) {
            if !wait.is_zero() {
                thread::sleep(wait);
            }
        }

        let ts = now_millis_since_epoch();
        self.rebuild_batch(ts);
        self.last_emit_end = Some(Instant::now());
        ts
    }

    /// Котировка из последнего выпущенного пакета (тот же `timestamp`, что у остальных в пакете).
    pub fn last_batch_quote(&self, ticker: &str) -> Option<StockQuote> {
        self.current_batch.get(ticker).cloned()
    }

    /// Единый timestamp последнего пакета, если уже был хотя бы один `advance_batch`.
    pub fn last_batch_timestamp(&self) -> Option<u64> {
        self.ticker_order
            .first()
            .and_then(|t| self.current_batch.get(t.as_str()))
            .map(|q| q.timestamp)
    }

    /// Все котировки последнего пакета в порядке [`Self::ticker_order`].
    pub fn last_batch_quotes(&self) -> Vec<StockQuote> {
        self.ticker_order
            .iter()
            .filter_map(|t| self.current_batch.get(t).cloned())
            .collect()
    }

    fn rebuild_batch(&mut self, timestamp: u64) {
        self.current_batch.clear();
        for ticker in &self.ticker_order {
            let price = self.last_price[ticker];
            let volume = self.last_volume[ticker];
            self.current_batch.insert(
                ticker.clone(),
                StockQuote {
                    ticker: ticker.clone(),
                    price,
                    volume,
                    timestamp,
                },
            );
        }
    }

    fn step_ticker(&mut self, ticker: &str) {
        let old_price = *self.last_price.get(ticker).expect("ticker in order");
        let old_volume = *self.last_volume.get(ticker).expect("ticker in order");

        let delta_percent = self.rng.next_i32_range(-DELTA_PRICE_BPS, DELTA_PRICE_BPS) as f64
            / DELTA_PRICE_BPS_DENOM;
        let mut new_price = old_price * (1.0 + delta_percent);
        if new_price <= 0.0 {
            new_price = f64::EPSILON;
        }

        let is_popular = POPULAR_TICKERS.contains(&ticker);
        let base_volume = if is_popular {
            POPULAR_VOLUME_BASE
        } else {
            DEFAULT_VOLUME_BASE
        };
        let pop_extra = if is_popular {
            POPULAR_VOLUME_EXTRA
        } else {
            DEFAULT_VOLUME_EXTRA
        };

        let rand_add = (self.rng.next_f64_01() * pop_extra as f64) as u32;
        let drift = (old_volume / VOLUME_DRIFT_DIVISOR) as i32;
        let drift_add = self.rng.next_i32_range(-drift, drift);

        let mut new_volume = base_volume.saturating_add(rand_add) as i32 + drift_add;
        if new_volume < 1 {
            new_volume = 1;
        }

        self.last_price.insert(ticker.to_string(), new_price);
        self.last_volume.insert(ticker.to_string(), new_volume as u32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static DEMO_TICKERS: &[&str] = &["AAPL", "ZZZ"];

    #[test]
    fn first_batch_same_timestamp_all_tickers() {
        let mut generator =
            QuoteGenerator::new_with_seed_interval_for(DEMO_TICKERS, 1, Duration::ZERO);
        generator.advance_batch();
        let ts_a = generator.last_batch_quote("AAPL").unwrap().timestamp;
        let ts_z = generator.last_batch_quote("ZZZ").unwrap().timestamp;
        assert_eq!(ts_a, ts_z);
    }

    #[test]
    fn unknown_ticker_not_in_batch() {
        let mut generator =
            QuoteGenerator::new_with_seed_interval_for(DEMO_TICKERS, 1, Duration::ZERO);
        generator.advance_batch();
        assert!(generator.last_batch_quote("UNKNOWN").is_none());
    }

    #[test]
    fn batch_quote_fields_valid() {
        let mut generator =
            QuoteGenerator::new_with_seed_interval_for(DEMO_TICKERS, 0xABCDEF, Duration::ZERO);
        generator.advance_batch();
        let q = generator.last_batch_quote("AAPL").unwrap();
        assert_eq!(q.ticker, "AAPL");
        assert!(q.price > 0.0);
        assert!(q.volume >= 1);
    }

    #[test]
    fn roundtrip_last_batch_quote_string() {
        let mut generator =
            QuoteGenerator::new_with_seed_interval_for(DEMO_TICKERS, 0xBEEF, Duration::ZERO);
        generator.advance_batch();
        let q = generator.last_batch_quote("ZZZ").unwrap();
        let parsed = StockQuote::from_string(&q.to_string()).unwrap();
        assert_eq!(parsed, q);
    }

    #[test]
    fn same_seed_same_first_batch_prices() {
        let mut a =
            QuoteGenerator::new_with_seed_interval_for(DEMO_TICKERS, 12345, Duration::ZERO);
        let mut b =
            QuoteGenerator::new_with_seed_interval_for(DEMO_TICKERS, 12345, Duration::ZERO);
        a.advance_batch();
        b.advance_batch();
        assert_eq!(
            a.last_batch_quote("AAPL").unwrap().price,
            b.last_batch_quote("AAPL").unwrap().price
        );
        assert_eq!(
            a.last_batch_quote("ZZZ").unwrap().volume,
            b.last_batch_quote("ZZZ").unwrap().volume
        );
    }

    #[test]
    fn second_batch_changes_prices() {
        let mut generator =
            QuoteGenerator::new_with_seed_interval_for(DEMO_TICKERS, 999, Duration::ZERO);
        generator.advance_batch();
        let p0 = generator.last_batch_quote("AAPL").unwrap().price;
        generator.advance_batch();
        let p1 = generator.last_batch_quote("AAPL").unwrap().price;
        assert!(
            (p1 - p0).abs() > 1e-9,
            "ожидали шаг блуждания между пакетами"
        );
    }

    #[test]
    fn last_batch_quotes_order_matches_ticker_order() {
        let mut generator =
            QuoteGenerator::new_with_seed_interval_for(DEMO_TICKERS, 1, Duration::ZERO);
        generator.advance_batch();
        let batch = generator.last_batch_quotes();
        assert_eq!(batch.len(), DEMO_TICKERS.len());
        assert_eq!(batch[0].ticker, "AAPL");
        assert_eq!(batch[1].ticker, "ZZZ");
    }
}
