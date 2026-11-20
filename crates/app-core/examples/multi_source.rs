//! Manual two-endpoint/four-source acceptance; no automated assertions.
use app_core::{
    api::{ConnectionState, Environment, HostProfile, SourceSpec},
    application::AppHandle,
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 5 {
        eprintln!("Usage: multi_source <alias-1> <alias-2> <file-1> <file-2>");
        return Ok(());
    }
    let app = AppHandle::new(PathBuf::from(format!(
        "/tmp/tailpro-multi-{:x}/settings.json",
        rand::random::<u64>()
    )));
    for (id, target) in [("one", &args[1]), ("two", &args[2])] {
        app.connect(HostProfile {
            id: id.into(),
            target: target.clone(),
            port: None,
            user: None,
            identity_agent: None,
            environment: Environment::Production,
        })
        .await?;
    }
    for _ in 0..100 {
        if app
            .status()
            .connections
            .iter()
            .filter(|c| matches!(c.state, ConnectionState::Connected))
            .count()
            == 2
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    for host in ["one", "two"] {
        for path in [&args[3], &args[4]] {
            let start = Instant::now();
            let source = app
                .start_source(host, SourceSpec::File { path: path.clone() })
                .await?;
            while start.elapsed() < Duration::from_secs(2) {
                if app
                    .status()
                    .sources
                    .iter()
                    .any(|s| s.id == source && s.state == "streaming")
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            println!(
                "warm_file_open_ms={} host={host}",
                start.elapsed().as_millis()
            );
        }
    }
    // Freeze the consumer, not ingestion. This does not emulate WebView heap or scroll.
    tokio::time::sleep(Duration::from_secs(30)).await;
    println!("status={}", serde_json::to_string(&app.status())?);
    println!("preview={}", serde_json::to_string(&app.preview().await?)?);
    let start = Instant::now();
    app.shutdown().await;
    println!("shutdown_ms={}", start.elapsed().as_millis());
    Ok(())
}
