//! Prefix scanning is bounded in bytes; structured/access profiles have separate budgets.
use super::decode::{Fields, ParseProfile, text_level};
use crate::model::{Level, evidence::Decoder, ingestion::{FieldOrigin, SeverityNamespace}};
use std::sync::LazyLock;
use time::{OffsetDateTime, PrimitiveDateTime, UtcOffset, format_description::{self, well_known::Rfc3339}};

static FORMATS: LazyLock<Vec<Vec<format_description::FormatItem<'static>>>> = LazyLock::new(|| {
    [
        "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond]",
        "[year]-[month]-[day] [hour]:[minute]:[second],[subsecond]",
        "[year]-[month]-[day] [hour]:[minute]:[second]",
        "[year]/[month]/[day] [hour]:[minute]:[second]",
        "[day padding:none] [month repr:short] [year] [hour]:[minute]:[second].[subsecond]",
        "[day padding:none] [month repr:short] [year] [hour]:[minute]:[second]",
    ].iter().map(|s|format_description::parse_borrowed::<2>(s).expect("static time format")).collect()
});
static APACHE: LazyLock<Vec<format_description::FormatItem<'static>>> = LazyLock::new(||format_description::parse_borrowed::<2>("[day]/[month repr:short]/[year]:[hour]:[minute]:[second] [offset_hour sign:mandatory][offset_minute]").expect("static Apache time"));
pub fn timestamp(s:&str,p:&ParseProfile)->Option<i128> {
    if let Ok(t)=OffsetDateTime::parse(s,&*APACHE) {return Some(t.unix_timestamp_nanos());}
    let offset=p.timezone_offset_minutes?;
    let offset=UtcOffset::from_whole_seconds(i32::from(offset)*60).ok()?;
    FORMATS.iter().find_map(|f|PrimitiveDateTime::parse(s,f).ok()).map(|t|t.assume_offset(offset).unix_timestamp_nanos())
}
fn prefix(text:&str)->&str {let mut n=text.len().min(256);while !text.is_char_boundary(n){n-=1;}&text[..n]}
fn find_time(text:&str,p:&ParseProfile,f:&mut Fields)->Option<usize> {
    let s=prefix(text);let mut starts=vec![0];
    starts.extend(s.char_indices().filter(|(_,c)|c.is_ascii_whitespace()||*c=='[').take(8).map(|(i,c)|i+c.len_utf8()));
    for start in starts {
        let rest=&s[start..];
        if !rest.as_bytes().first().is_some_and(u8::is_ascii_digit){continue;}
        for end in (1..=rest.len().min(80)).rev() {
            if !rest.is_char_boundary(end)||end<rest.len()&&!rest.as_bytes()[end].is_ascii_whitespace()&&rest.as_bytes()[end]!=b']'{continue;}
            let candidate=&rest[..end];
            let parsed=OffsetDateTime::parse(candidate,&Rfc3339).ok().map(|t|t.unix_timestamp_nanos()).or_else(||timestamp(candidate,p));
            if let Some(t)=parsed {
                f.time=Some(t.to_string());f.evidence.time_literal=Some(candidate.into());f.evidence.time_origin=FieldOrigin::Heuristic;return Some(start+end);
            }
            if candidate.len()>=19 && candidate.as_bytes().get(4)==Some(&b'-') && candidate.as_bytes().get(10)==Some(&b' ') { f.evidence.time_literal=Some(candidate.into()); }
        }
    }
    None
}
fn access(text:&str,p:&ParseProfile,f:&mut Fields)->bool {
    // The request is a quoted field after the timestamp, never search a URL for 5xx.
    let Some(open)=text.find('[') else{return false;};let Some(close)=text[open+1..].find(']').map(|i|i+open+1) else{return false;};
    let stamp=&text[open+1..close];let Some(t)=timestamp(stamp,p) else{return false;};
    let rest=text[close+1..].trim_start();if !rest.starts_with('"'){return false;}
    let mut escaped=false;let mut end=None;
    for (i,b) in rest.bytes().enumerate().skip(1) {if escaped{escaped=false;}else if b==b'\\'{escaped=true;}else if b==b'"'{end=Some(i);break;}}
    let Some(end)=end else{return false;};
    let Some(status)=rest[end+1..].split_whitespace().next().and_then(|s|s.parse::<u16>().ok()).filter(|s|(100..=599).contains(s)) else{return false;};
    f.time=Some(t.to_string());f.evidence.time_literal=Some(stamp.into());f.evidence.time_origin=FieldOrigin::Profile;
    f.evidence.http_status=Some(status);f.evidence.derived_http_error=status>=400;true
}
pub fn decode(text:&str,p:&ParseProfile,f:&mut Fields) {
    if p.access_log&&access(text,p,f){f.evidence.decoder=Decoder::Header;return;}
    let time_end=find_time(text,p,f);
    let header=prefix(text);
    let rest=time_end.map_or(header,|end|header[end..].trim_start_matches([']',' ','\t']));
    let first=rest.split_whitespace().next().unwrap_or("");
    let token=first.trim_matches(['[',']',':']);
    // A level must be a header token, not an arbitrary word in a sentence.
    if token.bytes().any(|b|b.is_ascii_alphabetic()) && token.bytes().all(|b|!b.is_ascii_lowercase()) {
        f.level=text_level(token);
        if f.level!=Level::Unknown { f.evidence.severity_literal=Some(token.into());f.evidence.severity_namespace=SeverityNamespace::Text;f.evidence.severity_origin=FieldOrigin::Heuristic; }
    }
    if text.starts_with("panic:")||text.starts_with("fatal error:")||text.starts_with("Traceback (most recent call last):")||text.starts_with("Error:") {
        f.level=Level::Error;f.evidence.severity_origin=FieldOrigin::Heuristic;
    }
    // Redis symbol interpretation requires pid:role + date/time header evidence.
    if let Some(end)=time_end && let Some((pid,role))=header.split_whitespace().next().and_then(|s|s.split_once(':'))
        && !pid.is_empty()&&pid.bytes().all(|b|b.is_ascii_digit())&&matches!(role,"M"|"S"|"C"|"X") {
        let symbol=header[end..].split_whitespace().next().unwrap_or("");
        f.level=match symbol {"."=>Level::Debug,"-"=>Level::Trace,"*"=>Level::Info,"#"=>Level::Warn,_=>f.level};
        if matches!(symbol,"."|"-"|"*"|"#") {f.evidence.severity_literal=Some(symbol.into());f.evidence.severity_origin=FieldOrigin::Heuristic;}
    }
    // RFC5424 payload from a selected file, not a syslog socket adapter.
    if let Some(end)=header.find('>') && header.starts_with('<') {
        let pri=header[1..end].parse::<u8>().ok();let mut tokens=header[end+1..].split_whitespace();
        if tokens.next()==Some("1") && let Some(stamp)=tokens.next() && let Ok(t)=OffsetDateTime::parse(stamp,&Rfc3339) && let Some(pri)=pri.filter(|p|*p<=191) {
            f.time=Some(t.unix_timestamp_nanos().to_string());f.evidence.time_literal=Some(stamp.into());f.evidence.time_origin=FieldOrigin::Heuristic;
            f.level=match pri%8 {0..=3=>Level::Error,4=>Level::Warn,5|6=>Level::Info,_=>Level::Debug};f.evidence.severity_namespace=SeverityNamespace::Syslog;f.evidence.severity_literal=Some((pri%8).to_string());f.evidence.severity_origin=FieldOrigin::Heuristic;
        }
    }
    if f.time.is_some()||f.level!=Level::Unknown {f.evidence.decoder=Decoder::Header;}
}
