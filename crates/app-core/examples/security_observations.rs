//! Run manually: cargo run -p app-core --example security_observations --features manual-fixtures -- <private-directory> [hold]
//! Prints observations, no automated assertions or pass/fail harness.
use app_core::{
    api::*,
    application::{AppHandle, bridge::Bridge},
};
use log_core::{model::Stream, parse::normalize};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::{fs, path::PathBuf, time::Duration};
fn request(source: &str, query: &str, limit: u32, cursor: Option<String>) -> ReadRequest {
    ReadRequest {
        sources: vec![source.into()],
        query: query.into(),
        limit: Some(limit),
        max_bytes: Some(4096),
        cursor,
        evidence_ref: None,
        ..Default::default()
    }
}
#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let directory = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("private output directory required")?,
    );
    fs::create_dir_all(&directory)?;
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
    let app = AppHandle::new(directory.join("settings.json"));
    let ingest = app.manual_source("incident").await?;
    let other = app.manual_source("ungranted").await?;
    for i in 1..=8 {
        ingest.append(normalize(format!(r#"{{"message":"incident row {i}","password":"CANARY_SECRET_{i}","level":"error"}}"#).as_bytes(),"incident",1,Stream::File,i,i)).await?;
    }
    other
        .append(normalize(
            b"UNAUTHORIZED_CANARY",
            "ungranted",
            1,
            Stream::File,
            1,
            1,
        ))
        .await?;
    let bridge = Bridge::start(app.clone()).await?;
    let enrollment = app.enroll(
        "manual-client".into(),
        vec!["incident".into()],
        true,
        bridge.directory.to_string_lossy().into_owned(),
    )?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(directory.join("enrollment.json"))?;
    std::io::Write::write_all(&mut file, &serde_json::to_vec(&enrollment)?)?;
    drop(file);
    let (grant, token) = (&enrollment.grant.id, &enrollment.token);
    let recent = app
        .agent_read(
            grant,
            token,
            "get_recent_logs",
            request("incident", "", 2, None),
        )
        .await?;
    println!(
        "recent events={} partial={} page={} baseline={}",
        recent.events.len(),
        !recent.complete,
        recent.next_cursor.is_some(),
        recent.observation_cursor.is_some()
    );
    let older = app
        .agent_read(
            grant,
            token,
            "get_recent_logs",
            request("incident", "", 2, recent.next_cursor),
        )
        .await?;
    println!(
        "older rows={:?}",
        older.events.iter().map(|e| &e.text).collect::<Vec<_>>()
    );
    let baseline = app
        .agent_read(
            grant,
            token,
            "get_recent_logs",
            request("incident", "incident", 0, None),
        )
        .await?;
    for i in 9..=15 {
        ingest
            .append(normalize(
                format!(r#"{{"message":"incident row {i}","token":"CANARY_INCREMENTAL"}}"#)
                    .as_bytes(),
                "incident",
                1,
                Stream::File,
                i,
                i,
            ))
            .await?;
    }
    let mut cursor = baseline.observation_cursor;
    let mut pages = 0;
    let mut count = 0;
    loop {
        let response = app
            .agent_read(
                grant,
                token,
                "search_logs",
                request("incident", "incident", 3, cursor),
            )
            .await?;
        count += response.events.len();
        pages += 1;
        println!(
            "observation page={pages} rows={:?} next={}",
            response.events.iter().map(|e| &e.text).collect::<Vec<_>>(),
            response.next_cursor.is_some()
        );
        if response.next_cursor.is_none() {
            break;
        }
        cursor = response.next_cursor;
    }
    println!("observation total={count}");
    let secret = app
        .agent_read(
            grant,
            token,
            "search_logs",
            request("incident", "CANARY_SECRET", 50, None),
        )
        .await?;
    println!("secret predicate matches={}", secret.events.len());
    let unauthorized = app
        .agent_read(
            grant,
            token,
            "search_logs",
            request("ungranted", "", 50, None),
        )
        .await;
    println!("ungranted rejected={}", unauthorized.is_err());
    ingest
        .append(normalize(
            b"=SUM(1,2)\nsecond line",
            "incident",
            1,
            Stream::File,
            16,
            16,
        ))
        .await?;
    let view = app.open_view().await?;
    let page = app
        .view_page(ViewRequest {
            view_id: view.clone(),
            generation: 1,
            query: "".into(),
            sources: vec!["incident".into()],
            before: None,
            anchor: None,
        })
        .await?;
    let selection = Selection {
        sources: vec!["incident".into()],
        event_ids: page.events.iter().map(|e| e.id.clone()).collect(),
        from_ms: None,
        to_ms: None,
        query: "".into(),
    };
    let mut p = app
        .prepare_export(selection.clone(), ExportFormat::Ndjson)
        .await?;
    if std::env::args().nth(2).as_deref() == Some("evict") {
        let payload = "x".repeat(32 * 1024);
        for i in 17..10_017 {
            ingest
                .append(normalize(
                    payload.as_bytes(),
                    "incident",
                    1,
                    Stream::File,
                    i,
                    i,
                ))
                .await?;
        }
        let expired = app
            .event_detail(DetailRequest {
                view_id: view.clone(),
                generation: 1,
                event_id: selection.event_ids[0].clone(),
                path: vec![],
                offset: 0,
                include_raw: false,
            })
            .await;
        println!("original event evicted={}", expired.is_err());
    }
    if std::env::args().nth(2).as_deref() == Some("expiry") {
        tokio::time::sleep(Duration::from_secs(61)).await;
        println!(
            "expired preview rejected={}",
            app.commit_export(&p.id, &p.digest, &directory.join("expired.ndjson"), false)
                .is_err()
        );
        p = app
            .prepare_export(selection.clone(), ExportFormat::Ndjson)
            .await?;
    }
    app.commit_export(&p.id, &p.digest, &directory.join("incident.ndjson"), false)?;
    println!(
        "export exact_bytes={} canary_present={}",
        fs::read(directory.join("incident.ndjson"))? == p.payload.as_bytes(),
        p.payload.contains("CANARY")
    );
    let p = app
        .prepare_export(selection.clone(), ExportFormat::Csv)
        .await?;
    println!(
        "CSV multiline neutralized={} quoting={}",
        p.payload.contains("\"'=SUM(1,2)\nsecond line\""),
        p.payload.contains("\"\"message\"\"")
    );
    app.commit_export(&p.id, &p.digest, &directory.join("incident.csv"), false)?;
    let p = app
        .prepare_export(selection.clone(), ExportFormat::Csv)
        .await?;
    app.cancel_prepared();
    println!(
        "cancelled commit rejected={}",
        app.commit_export(&p.id, &p.digest, &directory.join("cancel.csv"), false)
            .is_err()
    );
    let p = app.prepare_export(selection, ExportFormat::Csv).await?;
    println!(
        "disk failure surfaced={}",
        app.commit_export(
            &p.id,
            &p.digest,
            &directory.join("missing/incident.csv"),
            false
        )
        .is_err()
    );
    let selection = Selection {
        sources: vec!["incident".into()],
        event_ids: vec![],
        from_ms: None,
        to_ms: None,
        query: "".into(),
    };
    let p = app.prepare_export(selection, ExportFormat::Ndjson).await?;
    app.revoke(grant);
    println!(
        "revoke invalidates preview={}",
        app.commit_export(&p.id, &p.digest, &directory.join("revoked.ndjson"), false)
            .is_err()
    );
    println!(
        "revoked rejected={}",
        app.agent_read(
            grant,
            token,
            "list_sources",
            request("incident", "", 50, None)
        )
        .await
        .is_err()
    );
    // Independent client grants avoid rate-limit history from the observations above.
    for name in ["codex", "claude", "raw"] {
        let e = app.enroll(
            name.into(),
            vec!["incident".into()],
            true,
            bridge.directory.to_string_lossy().into_owned(),
        )?;
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(directory.join(format!("{name}.json")))?;
        std::io::Write::write_all(&mut f, &serde_json::to_vec(&e)?)?;
    }
    println!("ready private_directory={}", directory.display());
    if std::env::args().nth(2).as_deref() == Some("hold") {
        for _ in 0..600 {
            if directory.join("stop").exists() {
                break;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
    bridge.close().await;
    app.close_view(&view).await?;
    app.shutdown().await;
    Ok(())
}
