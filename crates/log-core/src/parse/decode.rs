//! Deterministic shared field mapping. Only declared paths are extracted;
//! borrowed RawValue validates the full record without retaining a JSON DOM.
use crate::model::{Level, evidence::{Decoder, ParseEvidence}, ingestion::{FieldOrigin, SeverityNamespace}};
use serde::{Deserialize, de::{self, MapAccess, Visitor}};
use serde_json::value::RawValue;
use std::{borrow::Cow, fmt};

pub use crate::model::ingestion::EpochUnit;
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ParseProfile {
    pub time_unit: Option<EpochUnit>,
    pub level_namespace: Option<SeverityNamespace>,
    pub timezone_offset_minutes: Option<i16>,
    pub raw_only: bool,
    pub access_log: bool,
}
pub struct Fields {
    pub message: Option<String>, pub level: Level, pub time: Option<String>,
    pub trace: Option<String>, pub order: Option<String>, pub user: Option<String>,
    pub duration: Option<f64>, pub malformed: bool, pub evidence: ParseEvidence,
}
impl Default for Fields {
    fn default() -> Self { Self { message: None, level: Level::Unknown, time: None, trace: None, order: None, user: None, duration: None, malformed: false, evidence: Default::default() } }
}
const PATHS: [&str; 24] = ["@timestamp","timestamp","time","ts","level","lvl","severity","severity_text","message","msg","error.message","err.message","trace_id","traceId","span_id","spanId","traceparent","order_id","user_id","duration_ms","http.response.status_code","severity_number","request_id","correlation_id"];
struct Object<'a,'de> { slots: &'a mut [Option<&'de RawValue>;24], conflict: &'a mut bool, prefix: &'a str }
impl<'de> Visitor<'de> for Object<'_,'de> {
    type Value = ();
    fn expecting(&self, f:&mut fmt::Formatter) -> fmt::Result { f.write_str("JSON object") }
    fn visit_map<A:MapAccess<'de>>(self, mut map:A) -> Result<(),A::Error> {
        while let Some(key) = map.next_key::<Cow<'de,str>>()? {
            let value = map.next_value::<&'de RawValue>()?;
            let path = if self.prefix.is_empty() { key.to_string() } else { format!("{}.{}",self.prefix,key) };
            if let Some(i) = PATHS.iter().position(|p| *p==path) {
                if self.slots[i].is_some() { *self.conflict = true; } else { self.slots[i]=Some(value); }
            }
            if matches!(path.as_str(),"error"|"err"|"http"|"http.response") && value.get().starts_with('{') {
                let mut d=serde_json::Deserializer::from_str(value.get());
                de::Deserializer::deserialize_map(&mut d,Object{slots:self.slots,conflict:self.conflict,prefix:&path}).map_err(de::Error::custom)?;
            }
        }
        Ok(())
    }
}
fn depth_ok(s:&str) -> bool {
    let (mut depth, mut quote, mut escape)=(0usize,false,false);
    for b in s.bytes() {
        if escape { escape=false; continue; }
        if quote && b==b'\\' { escape=true; continue; }
        if b==b'"' { quote=!quote; continue; }
        if !quote { match b { b'{'|b'['=>{depth+=1;if depth>64{return false;}}, b'}'|b']'=>depth=depth.saturating_sub(1), _=>{} } }
    }
    true
}
fn string(v:&RawValue) -> Option<String> { serde_json::from_str::<String>(v.get()).ok().filter(|s|s.len()<=crate::model::FRAME_BYTES) }
fn scalar(v:&RawValue) -> Option<String> { if v.get().starts_with('"') { string(v) } else if v.get().as_bytes().first().is_some_and(|b| b.is_ascii_digit()||*b==b'-') { Some(v.get().into()) } else {None} }
pub fn text_level(s:&str)->Level {
    if s.len()>16 { return Level::Unknown; }
    match s.to_ascii_lowercase().as_str() { "trace"=>Level::Trace,"debug"=>Level::Debug,"info"|"information"|"notice"=>Level::Info,"warn"|"warning"=>Level::Warn,"error"|"fatal"|"panic"|"crit"|"critical"|"severe"=>Level::Error,_=>Level::Unknown }
}
fn severity(value:&str, ns:Option<SeverityNamespace>)->Level {
    if let Some(ns)=ns && let Ok(n)=value.parse::<u8>() {
        return match ns {
            SeverityNamespace::Pino=>match n {10=>Level::Trace,20=>Level::Debug,30=>Level::Info,40=>Level::Warn,50|60=>Level::Error,_=>Level::Unknown},
            SeverityNamespace::Syslog=>match n {0..=3=>Level::Error,4=>Level::Warn,5|6=>Level::Info,7=>Level::Debug,_=>Level::Unknown},
            SeverityNamespace::OpenTelemetry=>match n {1..=4=>Level::Trace,5..=8=>Level::Debug,9..=12=>Level::Info,13..=16=>Level::Warn,17..=24=>Level::Error,_=>Level::Unknown},
            _=>Level::Unknown,
        };
    }
    text_level(value)
}
/// Checked decimal arithmetic, including pre-epoch fractions; no float round trip.
pub fn epoch_nanos(s:&str, unit:EpochUnit)->Option<i128> {
    if s.len()>48 { return None; }
    let negative=s.starts_with('-');let s=s.strip_prefix('-').unwrap_or(s);
    let (whole,fraction)=s.split_once('.').unwrap_or((s,""));
    if whole.is_empty()||!whole.bytes().all(|b|b.is_ascii_digit())||!fraction.bytes().all(|b|b.is_ascii_digit()) { return None; }
    let digits=match unit {EpochUnit::Seconds=>9,EpochUnit::Milliseconds=>6,EpochUnit::Microseconds=>3,EpochUnit::Nanoseconds=>0};
    if fraction.len()>digits { return None; }
    let scale=10_i128.pow(digits as u32);
    let n=whole.parse::<i128>().ok()?.checked_mul(scale)?.checked_add(if fraction.is_empty(){0}else{fraction.parse::<i128>().ok()?.checked_mul(10_i128.pow((digits-fraction.len()) as u32))?})?;
    if negative {n.checked_neg()} else {Some(n)}
}
pub fn timestamp(s:&str,p:&ParseProfile)->Option<String> {
    use time::{OffsetDateTime,format_description::well_known::Rfc3339};
    if s.len()>80 {return None;}
    if let Ok(t)=OffsetDateTime::parse(s,&Rfc3339) {return Some(t.unix_timestamp_nanos().to_string());}
    if let Some(unit)=p.time_unit && let Some(n)=epoch_nanos(s,unit) {return Some(n.to_string());}
    super::header::timestamp(s,p).map(|n|n.to_string())
}
fn map(values:&[Option<String>;24], numeric:&[bool;24], profile:&ParseProfile, f:&mut Fields) {
    for (i,value) in values[..4].iter().enumerate() {
        if let Some(value)=value {
            if f.evidence.time_literal.is_none() { f.evidence.time_literal=Some(value.clone()); f.evidence.time_path=Some(PATHS[i]); }
            if let Some(t)=timestamp(value,profile) {
                if f.time.is_some() { f.evidence.conflict=true; } else {f.time=Some(t);f.evidence.time_literal=Some(value.clone());f.evidence.time_path=Some(PATHS[i]);f.evidence.epoch_unit=profile.time_unit;f.evidence.time_origin=if numeric[i]{FieldOrigin::Profile}else{FieldOrigin::Explicit};}
            }
        }
    }
    for i in [4,5,6,7,21] { if let Some(value)=&values[i] {
        let ns=if i==21 {Some(SeverityNamespace::OpenTelemetry).filter(|_|profile.level_namespace==Some(SeverityNamespace::OpenTelemetry))} else {profile.level_namespace};
        let level=if numeric[i] && ns.is_none(){Level::Unknown}else{severity(value,ns)};
        if f.evidence.severity_literal.is_none() {f.evidence.severity_literal=Some(value.clone());}
        if level!=Level::Unknown {
            if f.level!=Level::Unknown {f.evidence.conflict=true;} else {f.level=level;f.evidence.severity_literal=Some(value.clone());f.evidence.severity_namespace=if numeric[i]{ns.unwrap_or(SeverityNamespace::Unknown)}else{SeverityNamespace::Text};f.evidence.severity_origin=if numeric[i]{FieldOrigin::Profile}else{FieldOrigin::Explicit};}
        }
    }}
    f.message=[8,9,10,11].iter().find_map(|i|values[*i].as_ref().map(|v|super::strip_ansi(v)));
    if values[8].is_some()&&values[9].is_some(){f.evidence.conflict=true;}
    f.trace=[12,13].iter().find_map(|i|values[*i].clone()).filter(|s|s.len()<=1024);
    if f.trace.is_none() { f.trace=values[16].as_deref().and_then(traceparent).map(str::to_owned); }
    f.order=values[17].clone().filter(|s|s.len()<=1024); f.user=values[18].clone().filter(|s|s.len()<=1024);
    f.duration=values[19].as_ref().and_then(|s|s.parse::<f64>().ok()).filter(|n|n.is_finite()&&*n>=0.);
    f.evidence.http_status=values[20].as_ref().and_then(|s|s.parse::<u16>().ok()).filter(|n|(100..=599).contains(n));
    f.evidence.derived_http_error=f.evidence.http_status.is_some_and(|s|s>=400);
}
fn traceparent(s:&str)->Option<&str> {
    let parts:Vec<_>=s.split('-').collect();
    if parts.len()!=4||parts[0]!="00"||parts[1].len()!=32||parts[2].len()!=16||parts[3].len()!=2 {return None;}
    if !parts.iter().all(|p|p.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b)))||parts[1].bytes().all(|b|b==b'0')||parts[2].bytes().all(|b|b==b'0'){return None;}
    Some(parts[1])
}
pub fn decode(text:&str, profile:&ParseProfile)->Fields {
    let mut f=Fields::default();if profile.raw_only{return f;}
    let text=text.trim();
    if text.starts_with(['{','[','"']) {
        f.evidence.decoder=Decoder::Json;
        if !depth_ok(text) {f.malformed=true;return f;}
        let Ok(raw)=serde_json::from_str::<&RawValue>(text) else {
            if text.starts_with('[') {super::header::decode(text,profile,&mut f);if f.time.is_some(){return f;}}
            f.malformed=true;return f;
        };
        if raw.get().starts_with('{') {
            let mut slots=[None;24];let mut d=serde_json::Deserializer::from_str(text);
            if de::Deserializer::deserialize_map(&mut d,Object{slots:&mut slots,conflict:&mut f.evidence.conflict,prefix:""}).is_err() {f.malformed=true;return f;}
            let values=slots.map(|v|v.and_then(scalar));let numeric=slots.map(|v|v.is_some_and(|v|!v.get().starts_with('"')));
            map(&values,&numeric,profile,&mut f);
        }
        return f;
    }
    if text.split_whitespace().next().is_some_and(|t|t.contains('=')) {
        f.evidence.decoder=Decoder::Logfmt;
        match logfmt(text) {Ok((v,conflict))=>{f.evidence.conflict=conflict;let numeric=std::array::from_fn(|i|v[i].as_ref().is_some_and(|s|s.parse::<i64>().is_ok()));map(&v,&numeric,profile,&mut f);},Err(())=>f.malformed=true}
        return f;
    }
    super::header::decode(text,profile,&mut f);
    f
}
fn logfmt(text:&str)->Result<([Option<String>;24],bool),()> {
    let mut values:[Option<String>;24]=Default::default();let mut conflict=false;
    pairs(text,|key,value|{if let Some(slot)=PATHS.iter().position(|p|*p==key){if values[slot].is_some(){conflict=true;}else{values[slot]=Some(value.into_owned());}}}).map_err(|_|())?;
    Ok((values,conflict))
}
/// Visit a logfmt record with one grammar for decode, lazy attributes and Inspector.
pub fn pairs<'a>(text:&'a str,mut visit:impl FnMut(&'a str,Cow<'a,str>))->Result<(), &'static str> {
    let b=text.as_bytes();let mut i=0;
    while i<b.len() {
        while i<b.len()&&b[i].is_ascii_whitespace(){i+=1;}if i==b.len(){break;}
        let start=i;while i<b.len()&&!b[i].is_ascii_whitespace()&&b[i]!=b'='&&b[i]!=b'"'{i+=1;}
        if i==start||b.get(i)!=Some(&b'='){return Err("Malformed logfmt key");}let key=&text[start..i];i+=1;
        let value;
        if b.get(i)==Some(&b'"') {
            let begin=i;i+=1;let mut escape=false;
            loop {let byte=*b.get(i).ok_or("Unclosed logfmt quote")?;i+=1;if escape {escape=false;}else if byte==b'\\'{escape=true;}else if byte==b'"'{break;}}
            value=Cow::Owned(serde_json::from_str(&text[begin..i]).map_err(|_|"Invalid logfmt escape")?);
            if i<b.len()&&!b[i].is_ascii_whitespace(){return Err("Malformed logfmt suffix");}
        } else {let begin=i;while i<b.len()&&!b[i].is_ascii_whitespace(){if b[i]==b'"'||b[i]==b'=' {return Err("Malformed logfmt value");}i+=1;}value=Cow::Borrowed(&text[begin..i]);}
        visit(key,value);
    }
    Ok(())
}
