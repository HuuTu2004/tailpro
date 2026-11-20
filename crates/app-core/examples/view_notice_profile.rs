//! Manual freeze/backpressure observations. No assertions or automated test harness.
use app_core::workers::EngineWorker;
use log_core::{model::Stream, parse::normalize};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cancel = CancellationToken::new();
    let engine = EngineWorker::start(cancel.clone());
    let mut sources = Vec::new();
    for id in ["nginx", "api", "worker", "quiet"] {
        sources.push(engine.register(id, 1).await?);
    }
    engine.open_view("manual".into()).await?;
    let baseline = engine.view_notice("manual".into(), None).await?;
    println!("baseline={}", serde_json::to_string(&baseline)?);
    let start = Instant::now();
    let producer = tokio::spawn(async move {
        let mut physical = 0;
        let mut tick = tokio::time::interval(Duration::from_millis(100));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        while start.elapsed() < Duration::from_secs(30) {
            tick.tick().await;
            for _ in 0..500 {
                physical += 1;
                let index = physical as usize % 4;
                let source = ["nginx", "api", "worker", "quiet"][index];
                let event = normalize(br#"{"message":"synthetic burst","trace_id":"req-42","duration_ms":120,"level":"error"}"#, source, 1, Stream::File, physical, 1);
                sources[index].append(event).await?;
            }
        }
        Ok::<_, app_core::api::Error>(physical)
    });
    // No polling or ACK for 30 seconds. Engine keeps one metadata notice, not rows.
    tokio::time::sleep(Duration::from_secs(30)).await;
    let physical = producer.await??;
    let unacked = engine.view_notice("manual".into(), None).await?;
    let caught_up = engine
        .view_notice("manual".into(), Some(baseline.revision))
        .await?;
    println!(
        "input={physical} elapsed_ms={} unacked_revision={} caught_up={}",
        start.elapsed().as_millis(),
        unacked.revision,
        serde_json::to_string(&caught_up)?
    );
    engine.close_view("manual".into()).await?;
    let app = app_core::application::AppHandle::new(std::env::temp_dir().join(format!(
        "tailpro-viewer-{:x}/settings.json",
        rand::random::<u64>()
    )));
    let id = app.open_view().await?;
    app.save_view(app_core::api::SavedView {
        name: "Incident".into(),
        query: "trace_id:req-42 AND duration>100ms".into(),
        sources: vec!["unavailable-source".into()],
    })?;
    println!(
        "saved_views={} sources_started={}",
        serde_json::to_string(&app.saved_views()?)?,
        app.status().sources.len()
    );
    let page = |generation| app_core::api::ViewRequest {
        view_id: id.clone(),
        generation,
        query: "".into(),
        sources: vec![],
        before: None,
        anchor: None,
    };
    let (old, new) = tokio::join!(app.view_page(page(1)), app.view_page(page(2)));
    println!(
        "old_generation={:?} new_generation={:?}",
        old.as_ref().map(|p| p.generation),
        new.as_ref().map(|p| p.generation)
    );
    let detail = app
        .event_detail(app_core::api::DetailRequest {
            view_id: id.clone(),
            generation: 1,
            event_id: "expired".into(),
            path: vec![],
            offset: 0,
            include_raw: true,
        })
        .await;
    println!("expired_detail={:?}", detail.as_ref().map(|d| &d.event_id));
    app.close_view(&id).await?;
    app.shutdown().await;
    cancel.cancel();
    Ok(())
}
