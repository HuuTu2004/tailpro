//! Operator-run synthetic observations, no assertions or test harness.
use app_core::application::redaction::{project, sanitize};
fn main() {
    let cases = [
        (
            "nested",
            r#"{"message":"incident","nested":{"password":"CANARY_NESTED","access_token":"CANARY_TOKEN"}}"#,
        ),
        (
            "url",
            r#"request https://example.invalid/x?token=CANARY_URL&token=CANARY_DUP&ok=1"#,
        ),
        (
            "encoded",
            r#"request https://example.invalid/?%2574oken=CANARY_ENCODED%26suffix"#,
        ),
        ("userinfo", "postgres://alice:CANARY_PASS@db.invalid/app"),
        (
            "bearer-unescape",
            r#"{"message":"Bearer CANARY_BEARER\u002eopaque"}"#,
        ),
        (
            "jwt",
            "received eyJhbGciOiJIUzI1NiJ9.eyJjYW5hcnkiOiJTRUNSRVQifQ.signature",
        ),
        ("jwe", "received aaa..bbb.ccc.ddd"),
        (
            "malformed-percent",
            "https://example.invalid/?token=CANARY_BAD%ZZ",
        ),
        (
            "embedded-json",
            r#"INFO payload {"password":"CANARY_EMBED"}"#,
        ),
        ("basic-auth", "Authorization: Basic CANARY_BASIC"),
        ("plain", "error request-42 status=500"),
    ];
    for (name, input) in cases {
        let out = sanitize(input);
        println!("{name}: incomplete={} output={}", out.incomplete, out.text);
    }
    let out = sanitize(&format!("token={}", "CANARY_HUGE".repeat(20_000)));
    println!("huge: incomplete={} output={}", out.incomplete, out.text);
    let event = |secret: &str| {
        log_core::parse::normalize(format!(r#"{{"message":"incident","password":{{"level":"{secret}","time":"{secret}","trace_id":"{secret}"}}}}"#).as_bytes(),"synthetic",1,log_core::model::Stream::File,1,1)
    };
    let (a, _) = project(&event("error"));
    let (b, _) = project(&event("info"));
    let p = log_core::query::QueryPlan::compile("error").unwrap();
    println!(
        "secret-only-change: same_raw={} same_predicate={} trace_a={:?} trace_b={:?} time_a={:?} time_b={:?}",
        a.raw == b.raw,
        p.matches(&a, "") == p.matches(&b, ""),
        a.trace_id,
        b.trace_id,
        a.event_time,
        b.event_time
    );
}
