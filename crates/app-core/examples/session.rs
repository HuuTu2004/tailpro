//! Interactive/manual acceptance helper. Uses a disposable host selected by the operator.
use app_core::{api::*, application::AppHandle};
use std::{path::PathBuf, time::Duration};
#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: session <SSH alias> <absolute synthetic file> [seconds]");
        return Ok(());
    }
    let spec = if let Some(id) = args[2].strip_prefix("docker:") {
        SourceSpec::Docker {
            container_id: id.into(),
            aliases: vec![],
        }
    } else if let Some(unit) = args[2].strip_prefix("journal:") {
        SourceSpec::Journal { unit: unit.into() }
    } else {
        SourceSpec::File {
            path: args[2].clone(),
        }
    };
    let seconds = args
        .get(3)
        .map(|s| s.parse::<u64>())
        .transpose()?
        .unwrap_or(10);
    let app = AppHandle::new(PathBuf::from(format!(
        "/tmp/tailpro-manual-{:x}/settings.json",
        rand::random::<u64>()
    )));
    println!("config={}", serde_json::to_string(&app.import_config()?)?);
    app.connect(HostProfile {
        id: "manual".into(),
        target: args[1].clone(),
        port: None,
        user: None,
        identity_agent: None,
        environment: Environment::Production,
    })
    .await?;
    for _ in 0..100 {
        let s = app.status();
        if s.connections.iter().any(|c| {
            matches!(
                c.state,
                ConnectionState::Connected | ConnectionState::NeedsAction
            )
        }) {
            println!("status={}", serde_json::to_string(&s)?);
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let started = std::time::Instant::now();
    let result = app
        .discover(DiscoveryRequest {
            host_id: "manual".into(),
            project_roots: vec![],
            docker: matches!(spec, SourceSpec::Docker { .. }),
            var_log: false,
            pm2_root: if matches!(spec, SourceSpec::File { .. }) {
                std::path::Path::new(&args[2])
                    .parent()
                    .map(|p| p.to_string_lossy().into_owned())
            } else {
                None
            },
            selected_pids: vec![],
            selected_units: vec![],
            config_files: vec![],
            config_roots: vec![],
            config_consent: false,
        })
        .await;
    println!(
        "discover_ms={} result={result:?}",
        started.elapsed().as_millis()
    );
    let source = app.start_source("manual", spec).await?;
    let until = std::time::Instant::now() + Duration::from_secs(seconds);
    while std::time::Instant::now() < until {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let preview = app.preview().await?;
        println!(
            "retained={} resident={} discarded={} last={}",
            preview.retained_rows,
            preview.resident_bytes,
            preview.discarded,
            serde_json::to_string(&preview.events)?
        );
    }
    app.stop_source(&source).await?;
    let started = std::time::Instant::now();
    app.shutdown().await;
    println!("shutdown_ms={}", started.elapsed().as_millis());
    Ok(())
}
