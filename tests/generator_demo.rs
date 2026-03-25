//! Интеграционная демонстрация: пакетные котировки с единым временем.
//!
//! Запуск с выводом в консоль:
//! `cargo test demo_quote_generator -- --nocapture`
//! `cargo test demo_one_ticker_five_steps -- --nocapture`

use std::time::Duration;
use utils::server::generator::QuoteGenerator;

#[test]
fn demo_quote_generator_prints_sample_lines() {
    let mut generator = QuoteGenerator::new_with_seed_and_interval(0xC0FFEE, Duration::ZERO);
    println!("\n=== Демо: 3 пакета, в пакете единый timestamp (tickers.txt) ===");
    for batch in 0..3 {
        let ts = generator.advance_batch();
        println!("  --- пакет {batch}, общий ts={ts} ---");
        for ticker in ["AAPL", "MSFT", "GOOGL"] {
            let q = generator.last_batch_quote(ticker).unwrap_or_else(|| {
                panic!("тикер {ticker} должен быть в tickers.txt");
            });
            println!("    {ticker}: {}", q.to_string());
        }
    }
    println!("=== конец демо ===\n");
}

#[test]
fn demo_one_ticker_five_steps() {
    let mut generator = QuoteGenerator::new_with_seed_and_interval(42, Duration::ZERO);
    println!("\n=== AAPL: 5 пакетов подряд (seed=42, interval=0 в тесте) ===");
    for step in 1..=5 {
        let ts = generator.advance_batch();
        let q = generator
            .last_batch_quote("AAPL")
            .expect("AAPL в tickers.txt");
        println!("  шаг {step} (ts={ts}): {}", q.to_string());
    }
    println!("=== конец ===\n");
}
