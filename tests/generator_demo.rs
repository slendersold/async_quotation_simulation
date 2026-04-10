//! Визуальная проверка генератора (`cargo test … -- --nocapture`).

use std::time::Duration;
use utils::server::generator::QuoteGenerator;

#[test]
fn demo_quote_generator_prints_sample_lines() {
    let mut generator = QuoteGenerator::new_with_seed_and_interval(0xC0FFEE, Duration::ZERO);
    println!("\n=== sample: 3 batches, shared timestamp per batch ===");
    for batch in 0..3 {
        let ts = generator.advance_batch();
        println!("  batch {batch}, ts={ts}");
        for ticker in ["AAPL", "MSFT", "GOOGL"] {
            let q = generator
                .last_batch_quote(ticker)
                .unwrap_or_else(|| panic!("missing ticker {ticker} in generator list"));
            println!("    {ticker}: {}", q.to_string());
        }
    }
    println!("=== end ===\n");
}

#[test]
fn demo_one_ticker_five_steps() {
    let mut generator = QuoteGenerator::new_with_seed_and_interval(42, Duration::ZERO);
    println!("\n=== AAPL: 5 batches, seed=42, zero emit interval in test ===");
    for step in 1..=5 {
        let ts = generator.advance_batch();
        let q = generator
            .last_batch_quote("AAPL")
            .expect("AAPL in default ticker list");
        println!("  step {step} (ts={ts}): {}", q.to_string());
    }
    println!("=== end ===\n");
}
