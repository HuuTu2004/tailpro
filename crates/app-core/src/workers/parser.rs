//! Session-scoped, explicitly imported parser configuration. Never reads files or executes code.
use crate::api::{Error, Result};
use log_core::{model::{Event, Level, evidence::Decoder, ingestion::{FieldOrigin, SeverityNamespace}}, parse::decode::{ParseProfile, text_level, timestamp}};
use regex::{Regex, RegexBuilder};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config { version:u32, #[serde(default)] parse:Parse }
#[derive(Default,Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Parse { profile:ParseProfile, rules:Vec<Rule> }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rule { id:String, source:String, pattern:String, time_format:Option<String> }
struct Compiled { id:String, regex:Regex }
pub struct Parser { pub revision:u64, pub profile:ParseProfile, rules:Vec<Compiled> }
impl Default for Parser {fn default()->Self{Self{revision:1,profile:Default::default(),rules:vec![]}}}
impl Parser {
    pub fn rule_count(&self)->usize{self.rules.len()}
    pub fn compile(config:&str,source:&str,revision:u64)->Result<Self> {
        if config.len()>64*1024 {return Err(Error::Limit("parser config 64 KiB"));}
        let config:Config=toml::from_str(config).map_err(|_|Error::Invalid("Invalid .tailpro.toml; previous configuration retained".into()))?;
        if config.version!=1 {return Err(Error::Invalid("Unsupported parser config version".into()));}
        if config.parse.rules.len()>4 {return Err(Error::Limit("4 candidate rules per source"));}
        if config.parse.profile.timezone_offset_minutes.is_some_and(|n|!(-1439..=1439).contains(&n)) {return Err(Error::Invalid("Timezone offset must be within one day".into()));}
        let mut rules=vec![];
        for rule in config.parse.rules {
            if rule.source!=source||rule.id.is_empty()||rule.id.len()>64||rules.iter().any(|r:&Compiled|r.id==rule.id) {return Err(Error::Invalid("Rule requires a unique bounded ID and the selected source ID".into()));}
            if rule.pattern.len()>2048 {return Err(Error::Limit("rule pattern 2 KiB"));}
            if rule.time_format.as_deref().is_some_and(|s|s!="rfc3339") {return Err(Error::Invalid("Unsupported time_format; use rfc3339 or profile time_unit/timezone".into()));}
            let regex=RegexBuilder::new(&rule.pattern).size_limit(96*1024).dfa_size_limit(32*1024).nest_limit(64).build().map_err(|_|Error::Invalid("Invalid/oversize regex; lookaround and backreferences are unsupported".into()))?;
            if regex.captures_len()>9||regex.capture_names().flatten().any(|n|!matches!(n,"time"|"level"|"msg"|"trace_id"|"order_id"|"user_id"|"duration_ms")) {return Err(Error::Invalid("Unsupported capture names or more than 8 captures".into()));}
            rules.push(Compiled{id:rule.id,regex});
        }
        Ok(Self{revision,profile:config.parse.profile,rules})
    }
    /// One capture search per candidate; sample spans are returned only to local preview.
    pub fn apply(&self,event:&mut Event)->Vec<(String,u32,u32)> {
        event.evidence.revision=self.revision;
        if self.profile.raw_only||event.fragment.is_some()||event.raw.len()>64*1024 {return vec![];}
        for rule in &self.rules {
            let Some(c)=rule.regex.captures(&event.raw) else{continue;};
            // Override rules replace the default extraction as a whole.
            event.level=Level::Unknown;event.event_time=None;event.trace_id=None;event.order_id=None;event.user_id=None;event.duration_ms=None;event.evidence=Default::default();event.evidence.decoder=Decoder::Rule;event.evidence.revision=self.revision;
            if let Some(v)=c.name("msg"){event.message=log_core::parse::strip_ansi(v.as_str());}
            if let Some(v)=c.name("level"){event.level=text_level(v.as_str());event.evidence.severity_origin=FieldOrigin::Profile;event.evidence.severity_namespace=SeverityNamespace::Text;event.evidence.severity_literal=Some(v.as_str().into());}
            if let Some(v)=c.name("time"){
                event.evidence.epoch_unit=self.profile.time_unit;event.evidence.time_literal=Some(v.as_str().into());event.evidence.time_origin=FieldOrigin::Profile;event.evidence.event_time_ns=timestamp(v.as_str(),&self.profile);
                event.event_time=event.evidence.event_time_ns.as_ref().and_then(|s|s.parse::<i128>().ok()).and_then(|n|time::OffsetDateTime::from_unix_timestamp_nanos(n).ok()).and_then(|t|t.format(&time::format_description::well_known::Rfc3339).ok());
            }
            event.trace_id=c.name("trace_id").filter(|m|m.len()<=1024).map(|m|m.as_str().into());event.order_id=c.name("order_id").filter(|m|m.len()<=1024).map(|m|m.as_str().into());event.user_id=c.name("user_id").filter(|m|m.len()<=1024).map(|m|m.as_str().into());event.duration_ms=c.name("duration_ms").and_then(|m|m.as_str().parse::<f64>().ok()).filter(|n|n.is_finite()&&*n>=0.);
            return rule.regex.capture_names().flatten().filter_map(|name|c.name(name).map(|m|(name.into(),m.start() as u32,m.end() as u32))).collect();
        }
        vec![]
    }
}
