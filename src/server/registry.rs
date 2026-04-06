//! Хаб рассылки пакетов котировок: один поток-генератор, много подписчиков с фильтром по тикерам.

use std::collections::HashSet;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

use crate::model::StockQuote;
use super::generator::QuoteGenerator;
use super::tickers;

/// Подписчик получает только котировки по своему множеству тикеров.
struct Subscriber {
    tickers: HashSet<String>,
    tx: mpsc::Sender<Vec<StockQuote>>,
}

struct HubInner {
    subscribers: Mutex<Vec<Subscriber>>,
}

/// Общий хаб: [`QuoteGenerator`] в отдельном потоке рассылает последний пакет всем подписчикам.
#[derive(Clone)]
pub struct QuoteHub {
    inner: Arc<HubInner>,
}

impl QuoteHub {
    /// Запускает поток генератора с фиксированным списком тикеров (`tickers.txt`) и интервалом выпуска.
    pub fn spawn_generator_thread(seed: u64, emit_interval: Duration) -> Self {
        let list = tickers::all_default();
        Self::spawn_generator_thread_for(seed, emit_interval, list)
    }

    /// То же, с явным статическим списком тикеров (тесты).
    pub fn spawn_generator_thread_for(
        seed: u64,
        emit_interval: Duration,
        tickers_list: &'static [&'static str],
    ) -> Self {
        let inner = Arc::new(HubInner {
            subscribers: Mutex::new(Vec::new()),
        });
        let inner_gen = inner.clone();
        thread::spawn(move || {
            let mut generator =
                QuoteGenerator::new_with_seed_interval_for(tickers_list, seed, emit_interval);
            loop {
                generator.advance_batch();
                let batch = generator.last_batch_quotes();
                let subs = inner_gen.subscribers.lock().unwrap();
                for sub in subs.iter() {
                    let filtered: Vec<StockQuote> = batch
                        .iter()
                        .filter(|q| sub.tickers.contains(&q.ticker))
                        .cloned()
                        .collect();
                    if filtered.is_empty() {
                        continue;
                    }
                    let _ = sub.tx.send(filtered);
                }
            }
        });
        Self { inner }
    }

    /// Подписка на отфильтрованные пакеты для UDP-стриминга.
    pub fn subscribe(&self, tickers: Vec<String>) -> mpsc::Receiver<Vec<StockQuote>> {
        let (tx, rx) = mpsc::channel();
        let set: HashSet<String> = tickers.into_iter().collect();
        self.inner
            .subscribers
            .lock()
            .unwrap()
            .push(Subscriber { tickers: set, tx });
        rx
    }
}
