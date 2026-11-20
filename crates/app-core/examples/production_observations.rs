//! Manually invoked synthetic observations, not an automated test harness.
use app_core::{
    api::*,
    application::{AppHandle, bridge::Bridge},
};
use log_core::{model::Stream, parse::normalize};
use std::{
    fs,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::PathBuf,
    time::Instant,
};
#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let directory = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("private fixture directory required")?,
    );
    fs::create_dir_all(&directory)?;
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
    let app = AppHandle::new(directory.join("settings.json"));
    for source in ["api", "worker", "db", "ungranted"] {
        let ingest = app.manual_source(source).await?;
        for i in 0..8 {
            let stack = if i < 6 {
                "at src/checkout.ts:42:3"
            } else {
                "at src/unrelated.ts:17:2"
            };
            let value = serde_json::json!({"level":"error", "message":format!("request failed {stack}"), "trace_id":"trace-demo", "order_id":"order-demo", "password":format!("SYNTHETIC_CANARY_{i}"), "duration_ms":if source=="api" {Some(25.0)}else{None}});
            ingest
                .append(normalize(
                    value.to_string().as_bytes(),
                    source,
                    1,
                    Stream::File,
                    i + 1,
                    1000 + i,
                ))
                .await?;
        }
    }
    let hints = app.manual_source("hints").await?;
    hints.append(normalize(serde_json::json!({"level":"error","message":"hint-canary at src/ok.ts:7:1 at ../bad.ts:8 at /tmp/abs.ts:9 at https://evil/path.ts:10", "revision":"different-event-revision"}).to_string().as_bytes(),"hints",1,Stream::File,1,2000)).await?;
    for source in ["api", "worker", "db", "hints"] {
        app.save_service_mapping(ServiceMapping {
            source: source.into(),
            service: if source == "db" { "db" } else { "checkout" }.into(),
            project: "demo".into(),
            aliases: vec![source.into()],
            repo_label: "demo-repo".into(),
            repo_root: "/declared-only/not-opened".into(),
            revision: Some("declared-rev-unverified".into()),
        })?;
    }
    let bridge = Bridge::start(app.clone()).await?;
    let enrolled = app.enroll(
        "phase4-observer".into(),
        vec!["api".into(), "worker".into(), "db".into()],
        true,
        bridge.directory.to_string_lossy().into_owned(),
    )?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(directory.join("enrollment.json"))?;
    serde_json::to_writer(&mut file, &enrolled)?;
    for (tool, req) in [
        (
            "locate_service",
            ReadRequest {
                service: Some("checkout".into()),
                ..Default::default()
            },
        ),
        (
            "locate_service",
            ReadRequest {
                service: Some("ungranted".into()),
                ..Default::default()
            },
        ),
        ("get_error_clusters", ReadRequest::default()),
        (
            "trace_id",
            ReadRequest {
                correlation: Some(Correlation::TraceId("trace-demo".into())),
                ..Default::default()
            },
        ),
        (
            "trace_id",
            ReadRequest {
                correlation: Some(Correlation::OrderId("order-demo".into())),
                from_ms: Some("1002".into()),
                to_ms: Some("1004".into()),
                ..Default::default()
            },
        ),
    ] {
        let start = Instant::now();
        let r = app
            .agent_read(&enrolled.grant.id, &enrolled.token, tool, req)
            .await?;
        println!(
            "tool={tool} bytes={} elapsed_us={} sources={} candidates={} ambiguous={} events={} clusters={:?} timeline={} complete={} gaps={:?}",
            serde_json::to_vec(&r)?.len(),
            start.elapsed().as_micros(),
            r.sources.len(),
            r.candidates.len(),
            r.ambiguous,
            r.events.len(),
            r.clusters
                .iter()
                .map(|g| g.observed_count)
                .collect::<Vec<_>>(),
            r.timeline.len(),
            r.complete,
            r.gaps
        );
        if tool == "get_error_clusters" {
            let context = app
                .agent_read(
                    &enrolled.grant.id,
                    &enrolled.token,
                    "inspect_context",
                    ReadRequest {
                        evidence_ref: Some(r.clusters[0].group_ref.clone()),
                        limit: Some(5),
                        ..Default::default()
                    },
                )
                .await?;
            println!(
                "inspect group events={} hints={:?}",
                context.events.len(),
                context
                    .code_hints
                    .iter()
                    .map(|h| (&h.relative_path, h.line))
                    .collect::<Vec<_>>()
            );
            fs::write(
                directory.join("clusters.json"),
                serde_json::to_vec_pretty(&r)?,
            )?;
        }
        if tool == "trace_id" && r.trace_ref.is_some() {
            let context = app
                .agent_read(
                    &enrolled.grant.id,
                    &enrolled.token,
                    "inspect_context",
                    ReadRequest {
                        evidence_ref: r.trace_ref.clone(),
                        limit: Some(10),
                        max_bytes: Some(32 * 1024),
                        ..Default::default()
                    },
                )
                .await?;
            println!(
                "inspect_trace sources={:?} events={} hints={}",
                context
                    .timeline
                    .iter()
                    .map(|p| p.source.as_str())
                    .collect::<std::collections::BTreeSet<_>>(),
                context.events.len(),
                context.code_hints.len()
            );
        }
        fs::write(
            directory.join(format!("{tool}.json")),
            serde_json::to_vec_pretty(&r)?,
        )?;
    }
    let denied = app
        .agent_read(
            &enrolled.grant.id,
            &enrolled.token,
            "trace_id",
            ReadRequest {
                sources: vec!["ungranted".into()],
                correlation: Some(Correlation::TraceId("trace-demo".into())),
                ..Default::default()
            },
        )
        .await;
    println!("outside_scope_error={}", denied.is_err());
    let start = Instant::now();
    let raw = app
        .agent_read(
            &enrolled.grant.id,
            &enrolled.token,
            "search_logs",
            ReadRequest {
                query: "level:error".into(),
                ..Default::default()
            },
        )
        .await?;
    println!(
        "raw_search bytes={} elapsed_us={} events={}",
        serde_json::to_vec(&raw)?.len(),
        start.elapsed().as_micros(),
        raw.events.len()
    );
    let hint_grant = app.enroll(
        "hint-observer".into(),
        vec!["hints".into()],
        true,
        bridge.directory.to_string_lossy().into_owned(),
    )?;
    let found = app
        .agent_read(
            &hint_grant.grant.id,
            &hint_grant.token,
            "get_recent_logs",
            ReadRequest::default(),
        )
        .await?;
    let context = app
        .agent_read(
            &hint_grant.grant.id,
            &hint_grant.token,
            "inspect_context",
            ReadRequest {
                evidence_ref: Some(found.events[0].evidence_ref.clone()),
                ..Default::default()
            },
        )
        .await?;
    println!(
        "hostile_stack_hints={:?}",
        context
            .code_hints
            .iter()
            .map(|h| (&h.relative_path, h.line))
            .collect::<Vec<_>>()
    );
    app.revoke(&hint_grant.grant.id);
    let mut cp = app_core::adapters::sources::Checkpoints::new(9);
    let first = cp.allocate(app_core::adapters::sources::Position::Journal(
        "cursor-A".into(),
    ))?;
    let second = cp.allocate(app_core::adapters::sources::Position::Journal(
        "cursor-B".into(),
    ))?;
    cp.ack(9, &[second]);
    println!("journal_out_of_order={:?}", cp.position);
    cp.ack(8, &[first]);
    println!("journal_old_generation={:?}", cp.position);
    cp.ack(9, &[first]);
    println!("journal_contiguous={:?}", cp.position);
    if std::env::args().any(|s| s == "caps") {
        let ingest = app.manual_source("caps").await?;
        for i in 0..740 {
            let msg = if i < 600 {
                "same error".to_string()
            } else {
                format!("distinct stack frame {i}")
            };
            ingest
                .append(normalize(
                    serde_json::json!({"level":"error","message":msg})
                        .to_string()
                        .as_bytes(),
                    "caps",
                    1,
                    Stream::File,
                    i + 1,
                    3000 + i,
                ))
                .await?;
        }
        let g = app.enroll(
            "cap-observer".into(),
            vec!["caps".into()],
            true,
            bridge.directory.to_string_lossy().into_owned(),
        )?;
        for from in [None, Some("3600".into())] {
            let r = app
                .agent_read(
                    &g.grant.id,
                    &g.token,
                    "get_error_clusters",
                    ReadRequest {
                        limit: Some(128),
                        max_bytes: Some(1024 * 1024),
                        from_ms: from,
                        ..Default::default()
                    },
                )
                .await?;
            println!(
                "cluster_caps groups={} represented={} bytes={} complete={} truncated={} labelled={}",
                r.clusters.len(),
                r.clusters.iter().map(|c| c.observed_count).sum::<u32>(),
                serde_json::to_vec(&r)?.len(),
                r.complete,
                r.truncated,
                r.clusters.iter().all(|c| c.incomplete)
            );
        }
        app.revoke(&g.grant.id);
        let ingest = app.manual_source("gap").await?;
        let mut marker = normalize(
            b"Journal cursor vacuumed; continuity gap",
            "gap",
            1,
            Stream::System,
            1,
            4000,
        );
        marker.quality.uncertain = true;
        ingest.append(marker).await?;
        ingest
            .append(normalize(
                b"ERROR retained after gap",
                "gap",
                1,
                Stream::Journal,
                2,
                4001,
            ))
            .await?;
        let g = app.enroll(
            "gap-observer".into(),
            vec!["gap".into()],
            true,
            bridge.directory.to_string_lossy().into_owned(),
        )?;
        let r = app
            .agent_read(
                &g.grant.id,
                &g.token,
                "get_error_clusters",
                ReadRequest::default(),
            )
            .await?;
        println!(
            "filtered_gap events={} complete={} gaps={:?}",
            r.events.len(),
            r.complete,
            r.gaps
        );
        app.revoke(&g.grant.id);
    }
    if std::env::args().any(|s| s == "hold") {
        let client = app.enroll(
            "phase4-mcp-client".into(),
            vec!["api".into(), "worker".into(), "db".into()],
            true,
            bridge.directory.to_string_lossy().into_owned(),
        )?;
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(directory.join("mcp-enrollment.json"))?;
        serde_json::to_writer(&mut f, &client)?;
        println!("Fixture ready for isolated clients; enrollment file is private");
        tokio::time::sleep(std::time::Duration::from_secs(600)).await;
    }
    app.revoke(&enrolled.grant.id);
    println!(
        "revoked_error={}",
        app.agent_read(
            &enrolled.grant.id,
            &enrolled.token,
            "get_error_clusters",
            ReadRequest::default()
        )
        .await
        .is_err()
    );
    bridge.close().await;
    app.shutdown().await;
    Ok(())
}
