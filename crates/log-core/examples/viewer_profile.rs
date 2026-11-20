//! Operator-run phase 2 observations, not automated assertions or a test runner.
use log_core::{
    engine::Engine,
    model::Stream,
    parse::normalize,
    query::{QueryPlan, ScanRequest, scan},
};
use std::time::{Duration, Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let count: usize = std::env::args().nth(1).unwrap_or("100000".into()).parse()?;
    let mut engine = Engine::new("viewer".into());
    for source in ["nginx", "api", "worker", "quiet"] {
        engine.register(source, 1)?;
    }
    let payload = "x".repeat(260);
    let start = Instant::now();
    for i in 0..count {
        let source = ["nginx", "api", "worker", "quiet"][i % 4];
        let raw = format!(
            r#"{{"message":"{payload}","trace_id":"req-{}","order_id":"order-{}","level":"{}","duration_ms":{}}}"#,
            (i / 4) % 1000,
            (i / 4) % 100,
            if i % 7 == 0 { "error" } else { "info" },
            i % 1000
        );
        engine.append(
            normalize(raw.as_bytes(), source, 1, Stream::File, i as u64 + 1, 1),
            1,
        )?;
    }
    let stats = engine.stats();
    println!(
        "input={count} ingest_ms={} rows={} resident={} accepted={} discarded={} evicted={}",
        start.elapsed().as_millis(),
        stats.live_rows,
        stats.resident_bytes,
        stats.counters.accepted,
        stats.counters.discarded,
        stats.counters.evicted
    );
    let snapshot = engine.snapshot()?;
    for query in [
        "",
        "trace_id:req-42",
        "order_id:order-7 AND level:error",
        "user_id:missing",
        "duration>200ms AND duration_ms<=1s",
        "level:\"error\"",
        "message:\"x x\"",
    ] {
        let plan = QueryPlan::compile(query)?;
        let mut times = Vec::new();
        let mut last = None;
        for _ in 0..25 {
            let start = Instant::now();
            let result = scan(
                &snapshot,
                &plan,
                ScanRequest {
                    before: None,
                    anchor: None,
                    sources: &[],
                    deadline: Instant::now() + Duration::from_secs(2),
                },
                |s| {
                    Some(if s == "worker" || s == "quiet" {
                        "beta"
                    } else {
                        "alpha"
                    })
                },
                || false,
            )?;
            times.push(start.elapsed().as_micros());
            last = Some(result);
        }
        times.sort_unstable();
        let result = last.unwrap();
        let sources: std::collections::BTreeSet<_> =
            result.events.iter().map(|e| e.source.as_str()).collect();
        println!(
            "query={query:?} p50_us={} p95_us={} max_us={} page={} scanned={} complete={} older={} sources={sources:?}",
            times[12],
            times[23],
            times[24],
            result.events.len(),
            result.scanned,
            result.complete,
            result.has_older
        );
        if let Some(oldest) = result.events.first() {
            let older = scan(
                &snapshot,
                &plan,
                ScanRequest {
                    before: Some(&oldest.id),
                    anchor: None,
                    sources: &[],
                    deadline: Instant::now() + Duration::from_secs(2),
                },
                |_| Some("alpha"),
                || false,
            )?;
            println!(
                "  older_page={} before={} last={:?}",
                older.events.len(),
                oldest.id,
                older.events.last().map(|e| &e.id)
            );
        }
    }
    for query in [
        "a OR b",
        "NOT a",
        "a AND",
        "duration>1",
        "unknown:x",
        "level:nope",
        "\"unclosed",
        "message:/foo/",
        "duration>NaNs",
    ] {
        println!("compile={query:?} result={:?}", QueryPlan::compile(query));
    }
    let plan = QueryPlan::compile("")?;
    let cancelled = scan(
        &snapshot,
        &plan,
        ScanRequest {
            before: None,
            anchor: None,
            sources: &[],
            deadline: Instant::now() + Duration::from_secs(2),
        },
        |_| Some("alpha"),
        || true,
    )?;
    let expired = scan(
        &snapshot,
        &plan,
        ScanRequest {
            before: None,
            anchor: Some("expired"),
            sources: &[],
            deadline: Instant::now() + Duration::from_secs(2),
        },
        |_| Some("alpha"),
        || false,
    )?;
    println!(
        "cancel_complete={} expired_anchor={}",
        cancelled.complete, expired.anchor_expired
    );
    let wide = format!(
        "{{{}}}",
        (0..5000)
            .map(|i| format!("\"key{i}\":{{\"nested\":{i}}}"))
            .collect::<Vec<_>>()
            .join(",")
    );
    for offset in [0, 200, 4800] {
        let (children, more) = log_core::inspect::children(&wide, &[], offset)?;
        println!(
            "inspector_offset={offset} children={} more={more}",
            children.len()
        );
    }
    let (children, _) = log_core::inspect::children(&wide, &[42], 0)?;
    println!(
        "inspector_nested={:?}",
        children.iter().map(|n| (&n.key, n.raw)).collect::<Vec<_>>()
    );
    Ok(())
}
