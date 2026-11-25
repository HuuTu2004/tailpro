//! Manually invoked schema walkthrough; prints observations without a test harness.
use log_core::{
    engine::Disposition,
    model::{
        Stream,
        ingestion::{
            EventTimes, FieldOrigin, OriginalPayload, PayloadRange, SCHEMA_VERSION,
            SeverityNamespace, UnixNanos,
        },
    },
    parse::normalize,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("ingestion_schema_version={SCHEMA_VERSION}");
    let event = UnixNanos::new("1795194001123456789").ok_or("event nanos")?;
    let transport = UnixNanos::new("1795194001223456789").ok_or("transport nanos")?;
    let observed = UnixNanos::new("1795194001323456789").ok_or("observed nanos")?;
    for times in [
        EventTimes {
            event: Some(event),
            transport: Some(transport),
            observed,
        },
        EventTimes {
            event: None,
            transport: Some(transport),
            observed,
        },
        EventTimes {
            event: None,
            transport: None,
            observed,
        },
    ] {
        println!(
            "times={} display={:?}",
            serde_json::to_string(&times)?,
            times.display()
        );
    }
    for value in [
        "0",
        "-1",
        "+1",
        "01",
        "-0",
        "1.5",
        "9007199254740993",
        "170141183460469231731687303715884105727",
        "170141183460469231731687303715884105728",
        "-170141183460469231731687303715884105728",
        "-170141183460469231731687303715884105729",
    ] {
        println!(
            "canonical_nanos input={value:?} result={}",
            serde_json::to_string(&UnixNanos::new(value))?
        );
    }

    let first = PayloadRange::new(0, 4).ok_or("first range")?;
    let second = PayloadRange::new(4, 8).ok_or("second range")?;
    let bytes = [b'a', 0xff, b'\r', 0];
    let payload = OriginalPayload::new(&bytes, first, 4).ok_or("original bytes")?;
    println!(
        "payload bytes={:?} range={:?} same_allocation={} next_contiguous={} inverted={:?} wrong_length={} over_cap={}",
        payload.bytes(),
        payload.range(),
        payload.bytes().as_ptr() == bytes.as_ptr(),
        first.followed_by(second),
        PayloadRange::new(8, 4),
        OriginalPayload::new(&bytes[..3], first, 4).is_some(),
        OriginalPayload::new(&bytes, first, 3).is_some(),
    );
    println!(
        "severity_namespaces={} field_origin={} dispositions={:?}/{:?}",
        serde_json::to_string(&[
            SeverityNamespace::Pino,
            SeverityNamespace::Syslog,
            SeverityNamespace::OpenTelemetry,
            SeverityNamespace::Unknown,
        ])?,
        serde_json::to_string(&FieldOrigin::Unknown)?,
        Disposition::Accepted,
        Disposition::Gap,
    );

    // These observations intentionally expose the legacy parser's remaining gates.
    // The schema primitives above do not implement phase 6.3/6.4 extraction.
    for (name, input, stream) in [
        (
            "root-before-nested",
            r#"{"level":"info","payload":{"level":"error"},"msg":"synthetic"}"#,
            Stream::File,
        ),
        (
            "nested-before-root",
            r#"{"payload":{"level":"error"},"level":"info","msg":"synthetic"}"#,
            Stream::Stdout,
        ),
        (
            "numeric-time-and-level",
            r#"{"time":1795194001123,"level":50,"msg":"synthetic"}"#,
            Stream::Journal,
        ),
        (
            "decimal-ts",
            r#"{"ts":1795194001.123456789,"level":"warn","msg":"synthetic"}"#,
            Stream::File,
        ),
    ] {
        let normalized = normalize(input.as_bytes(), "synthetic", 1, stream, 1, 0);
        println!(
            "legacy_observation case={name} stream={stream:?} level={:?} time={:?} malformed={}",
            normalized.level, normalized.event_time, normalized.quality.malformed,
        );
    }
    println!(
        "model_sizes event={} times={} payload_view={}",
        std::mem::size_of::<log_core::model::Event>(),
        std::mem::size_of::<EventTimes<'_>>(),
        std::mem::size_of::<OriginalPayload<'_>>(),
    );
    Ok(())
}
