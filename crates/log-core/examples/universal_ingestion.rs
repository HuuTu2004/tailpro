//! Operator-run observations against a checked-in synthetic manifest; no test runner/assertions.
use log_core::{engine::Engine,model::{Stream,RecordEnding},parse::{normalize_with,raw,Framer,Assembler,decode::ParseProfile},query::{QueryPlan,LocalMatcher}};
use std::time::Instant;
fn main()->Result<(),Box<dyn std::error::Error>> {
    let manifest:serde_json::Value=serde_json::from_str(include_str!("../../../fixtures/universal-ingestion/manifest.json"))?;
    for case in manifest["cases"].as_array().ok_or("manifest cases")? {
        let profile:ParseProfile=serde_json::from_value(case["profile"].clone())?;
        let e=normalize_with(case["input"].as_str().ok_or("input")?.as_bytes(),"corpus",1,Stream::Stderr,1,1,&profile);
        println!("{}",serde_json::json!({"id":case["id"],"expected":case["expected"],"observed":{"level":e.level,"message":e.message,"time_ns":e.evidence.event_time_ns,"evidence":e.evidence,"malformed":e.quality.malformed,"duration":e.duration_ms}}));
    }
    let mut input=vec![b'x';128*300+5];input[127]=0xff;input[128]=0;input[129]=0xe2;input[130]=0x82;input[131]=0xac;
    input[255..264].copy_from_slice(b"CROSS-HIT");input.extend_from_slice(b"\r\npartial");
    let mut framer=Framer::bounded(128);let mut reconstructed=Vec::new();let mut count=0;let mut hits=0;
    let plan=QueryPlan::compile("CROSS-HIT")?;let mut matcher=LocalMatcher::new(&plan);
    let mut engine=Engine::new("raw".into());engine.register("raw",1)?;
    for chunk in input.chunks(37) {
        let mut frames=vec![];framer.push(chunk,|f|frames.push(f))?;
        for f in frames {
            reconstructed.extend_from_slice(&f.bytes);reconstructed.extend_from_slice(f.ending.bytes());
            count+=1;let mut e=raw(f.bytes,"raw",1,Stream::File,count,0);e.ending=f.ending;e.fragment=f.fragment;e.quality.fragmented=e.fragment.is_some();
            hits+=usize::from(matcher.matches(&e,"host"));engine.append(e,1)?;
        }
    }
    if let Some(f)=framer.interrupt(){reconstructed.extend_from_slice(&f.bytes);reconstructed.extend_from_slice(f.ending.bytes());count+=1;let mut e=raw(f.bytes,"raw",1,Stream::File,count,0);e.ending=f.ending;e.quality.incomplete=true;engine.append(e,1)?;}
    let snapshot=engine.snapshot()?;
    println!("{}",serde_json::json!({"id":"U-RAW-01","same_bytes":input==reconstructed,"units":count,"boundary_hits":hits,"metrics":snapshot.source_metrics}));
    for (name,lines) in [
        ("go",vec!["panic: failed","goroutine 1 [running]:","main.main()","\t/app/main.go:12"]),
        ("python",vec!["Traceback (most recent call last):","  File \"app.py\", line 1","ValueError: failure"]),
        ("node",vec!["TypeError: failure","    at run (app.js:12:1)"]),
        ("java",vec!["Exception in thread \"main\" java.lang.IllegalStateException: failed","\tat app.Main.run(Main.java:12)","Caused by: java.io.IOException: failed","\t... 1 more"]),
        ("php",vec!["PHP Fatal error: Uncaught Exception","Stack trace:","#0 /app/index.php(12): run()","  thrown in /app/index.php on line 12"]),
        ("rust",vec!["thread 'main' panicked at src/main.rs:12:5:","stack backtrace:","   0: rust_begin_unwind","             at /rust/src/panicking.rs:12"]),
    ] {
        let mut assembler=Assembler::default();let mut ready=vec![];
        for (i,line) in lines.iter().enumerate(){let mut e=log_core::parse::normalize(line.as_bytes(),name,1,Stream::Stdout,i as u64+1,0);e.ending=RecordEnding::Lf;ready.extend(assembler.push(e,Instant::now()));}
        ready.extend(assembler.flush(Instant::now()));
        println!("{}",serde_json::json!({"id":"U-ML-01","language":name,"input_lines":lines.len(),"rows":ready.len(),"physical":ready.iter().map(|e|e.physical.len()).collect::<Vec<_>>(),"raw_bytes":ready.iter().map(|e|e.raw.bytes().len()+e.ending.bytes().len()).sum::<usize>()}));
    }
    let mut overflow=Framer::at(u64::MAX);println!("offset_overflow={:?}",overflow.push(b"x",|_|{}));
    Ok(())
}
