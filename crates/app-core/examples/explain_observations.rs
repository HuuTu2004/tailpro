//! Operator-run HTTP policy observations; endpoint must be a disposable Ollama-shaped fixture.
use app_core::{api::*, application::AppHandle};
use log_core::{model::Stream, parse::normalize};
#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let endpoint = std::env::args()
        .nth(1)
        .ok_or("literal-loopback /api/chat endpoint required")?;
    let app = AppHandle::new(std::env::temp_dir().join(format!(
        "tp3-explain-{:x}/settings.json",
        rand::random::<u64>()
    )));
    let source = app.manual_source("incident").await?;
    source
        .append(normalize(
            br#"{"message":"error incident","token":"CANARY_EXPLAIN_SECRET"}"#,
            "incident",
            1,
            Stream::File,
            1,
            1,
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
    println!("default_disabled={}", app.explain_provider().is_none());
    app.configure_explainer(Some(ExplainProvider::Ollama {
        endpoint: endpoint.clone(),
        model: "synthetic-local-fixture".into(),
    }))?;
    let p = app.prepare_explanation(selection.clone()).await?;
    println!(
        "preview_canary={} without_consent_rejected={}",
        p.payload.contains("CANARY"),
        app.explain(&p.id, &p.digest, false).await.is_err()
    );
    if std::env::args().nth(2).as_deref() == Some("cancel") {
        let app = app.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            app.cancel_prepared();
        });
    }
    let answer = app.explain(&p.id, &p.digest, true).await;
    println!(
        "provider_result={:?}",
        answer.map(|a| (a.untrusted, a.evidence.len(), a.hypothesis))
    );
    let p = app.prepare_explanation(selection).await?;
    app.configure_explainer(None)?;
    println!(
        "provider_change_rejected={}",
        app.explain(&p.id, &p.digest, true).await.is_err()
    );
    println!(
        "dns_local_rejected={}",
        app.configure_explainer(Some(ExplainProvider::Ollama {
            endpoint: "http://localhost:11434/api/chat".into(),
            model: "fixture".into()
        }))
        .is_err()
    );
    println!(
        "http_cloud_rejected={}",
        app.configure_explainer(Some(ExplainProvider::Byok {
            endpoint: "http://example.invalid/v1/chat/completions".into(),
            model: "fixture".into()
        }))
        .is_err()
    );
    app.configure_explainer(Some(ExplainProvider::Byok {
        endpoint: "https://example.invalid/v1/chat/completions".into(),
        model: "fixture".into(),
    }))?;
    println!(
        "os_store_result={:?}",
        app.store_provider_key("SYNTHETIC_KEY_NOT_A_CREDENTIAL".into())
            .await
    );
    app.close_view(&view).await?;
    app.shutdown().await;
    Ok(())
}
