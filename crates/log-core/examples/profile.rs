//! Manual profiling driver. Emits observations; no test harness or assertions.
use log_core::{
    engine::Engine,
    model::Stream,
    parse::{Assembler, Framer, normalize},
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
struct Allocations;
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
fn charge(bytes: usize) {
    ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    let current = LIVE.fetch_add(bytes, Ordering::Relaxed) + bytes;
    PEAK.fetch_max(current, Ordering::Relaxed);
}
// Profiling only. Every operation delegates unchanged to the system allocator.
unsafe impl GlobalAlloc for Allocations {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            charge(layout.size());
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        unsafe { System.dealloc(p, layout) };
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
    }
    unsafe fn realloc(&self, p: *mut u8, layout: Layout, new: usize) -> *mut u8 {
        let pointer = unsafe { System.realloc(p, layout, new) };
        if !pointer.is_null() {
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
            charge(new);
        }
        pointer
    }
}
#[global_allocator]
static GLOBAL: Allocations = Allocations;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let count: u64 = std::env::args().nth(1).unwrap_or("100000".into()).parse()?;
    let pace: u64 = std::env::args().nth(2).unwrap_or("0".into()).parse()?;
    let pin = std::env::args().any(|arg| arg == "pin");
    let mut pinned = None;
    let mut engine = Engine::new("synthetic".into());
    for source in ["hot", "quiet-a", "quiet-b", "quiet-c"] {
        engine.register(source, 1)?;
    }
    let (initial_snapshot, mut consumer) = engine.capture()?;
    drop(initial_snapshot);
    let payload = format!(
        r#"{{"level":"info","message":"{}","duration_ms":12,"trace_id":"synthetic"}}"#,
        "x".repeat(430)
    );
    let start = Instant::now();
    for n in 0..count {
        let event = normalize(payload.as_bytes(), "hot", 1, Stream::Stdout, n, n);
        engine.append(event, 1)?;
        if pin && n == 166_000 {
            pinned = Some(engine.snapshot()?);
            println!("snapshot pinned at {n}");
        }
        if pace > 0 && n % 100 == 99 {
            let due = std::time::Duration::from_secs_f64((n + 1) as f64 / pace as f64);
            if let Some(wait) = due.checked_sub(start.elapsed()) {
                std::thread::sleep(wait);
            }
        }
        if n % 7500 == 0 {
            for source in ["quiet-a", "quiet-b", "quiet-c"] {
                engine.append(normalize(b"quiet", source, 1, Stream::Stdout, n, n), 1)?;
            }
        }
    }
    println!(
        "ingest_count={count} elapsed_ms={} stats={}",
        start.elapsed().as_millis(),
        serde_json::to_string(&engine.stats())?
    );
    if pinned.is_some() {
        drop(pinned.take());
        println!(
            "after_pin_release={}",
            serde_json::to_string(&engine.stats())?
        );
    }
    let before = Instant::now();
    let snapshot = engine.snapshot()?;
    let recent = Instant::now();
    let events = snapshot.recent(50)?;
    println!(
        "recent_50_us={} rows={}",
        recent.elapsed().as_micros(),
        events.len()
    );
    let changes = engine.catch_up_subscription(&mut consumer)?;
    println!(
        "catch_up_complete={} changes={} fence={}",
        changes.complete,
        changes.changes.len(),
        changes.fence
    );
    let mut counts = std::collections::BTreeMap::new();
    for event in snapshot.events()? {
        *counts.entry(event.source.clone()).or_insert(0usize) += 1;
    }
    println!(
        "snapshot_scan_ms={} retained={counts:?}",
        before.elapsed().as_millis()
    );
    let mut framer = Framer::default();
    let mut frames = 0;
    framer.push(&vec![b'x'; 1024 * 1024], |_| frames += 1).expect("offset");
    framer.push(b"\n", |f| {
        println!(
            "oversize bytes={} truncated={} end={}",
            f.bytes.len(),
            f.truncated,
            f.end_offset
        )
    }).expect("offset");
    let mut assembler = Assembler::default();
    for (n, line) in [
        "Traceback (most recent call last):",
        "  File \"app.py\", line 1",
        "ValueError: synthetic",
        "INFO next header",
    ]
    .iter()
    .enumerate()
    {
        for event in assembler.push(
            normalize(line.as_bytes(), "demo", 1, Stream::Stdout, n as u64, 0),
            Instant::now(),
        ) {
            println!(
                "normalized={} {}",
                format_args!("{:?}", event.level),
                event.message
            );
        }
    }
    for event in assembler.flush(Instant::now()) {
        println!("normalized={:?} {}", event.level, event.message);
    }
    for bytes in [
        include_str!("../../../fixtures/synthetic.ndjson")
            .lines()
            .nth(1)
            .unwrap()
            .to_string(),
        format!("{}{}", "[".repeat(80), "]".repeat(80)),
        "{malformed".into(),
    ] {
        let event = normalize(bytes.as_bytes(), "fixture", 1, Stream::Stdout, 1, 0);
        println!(
            "parse_fixture message={:?} quality={:?}",
            event.message.chars().take(100).collect::<String>(),
            event.quality
        );
    }
    println!(
        "unused_frames={frames} allocation_calls={} live_allocated_bytes={} peak_allocated_bytes={} payload_bytes={}",
        ALLOCATIONS.load(Ordering::Relaxed),
        LIVE.load(Ordering::Relaxed),
        PEAK.load(Ordering::Relaxed),
        payload.len()
    );
    Ok(())
}
