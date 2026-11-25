//! Stateless lazy JSON projection. RawValue borrows the frame; no retained JSON tree.
use serde::de::{self, DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde_json::value::RawValue;
use std::fmt;

pub struct Child<'a> {
    pub index: usize,
    pub key: String,
    pub raw: &'a str,
}
struct Children {
    offset: usize,
    limit: usize,
}
impl<'de> DeserializeSeed<'de> for Children {
    type Value = (Vec<Child<'de>>, bool);
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Children {
    type Value = (Vec<Child<'de>>, bool);
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("JSON container")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut children = Vec::new();
        let mut i = 0;
        while let Some(key) = map.next_key::<String>()? {
            if i >= self.offset && children.len() < self.limit {
                let raw = map.next_value::<&'de RawValue>()?;
                children.push(Child {
                    index: i,
                    key,
                    raw: raw.get(),
                });
            } else {
                map.next_value::<IgnoredAny>()?;
            }
            i += 1;
        }
        Ok((children, i > self.offset.saturating_add(self.limit)))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut children = Vec::new();
        let mut i = 0;
        loop {
            if i >= self.offset && children.len() < self.limit {
                let Some(raw) = seq.next_element::<&'de RawValue>()? else {
                    break;
                };
                children.push(Child {
                    index: i,
                    key: i.to_string(),
                    raw: raw.get(),
                });
            } else if seq.next_element::<IgnoredAny>()?.is_none() {
                break;
            }
            i += 1;
        }
        Ok((children, i > self.offset.saturating_add(self.limit)))
    }
}
/// Integer path addresses object entries/array elements without accepting log-supplied code.
pub fn children<'a>(
    raw: &'a str,
    path: &[usize],
    offset: usize,
) -> Result<(Vec<Child<'a>>, bool), String> {
    if path.len() > 64 || offset > 262_144 {
        return Err("Inspector path budget".into());
    }
    let mut selected = raw;
    for &index in path {
        let (nodes, _) = Children {
            offset: index,
            limit: 1,
        }
        .deserialize(&mut serde_json::Deserializer::from_str(selected))
        .map_err(|_| "Not a JSON container or expired path")?;
        selected = nodes.first().ok_or("JSON path missing")?.raw;
    }
    Children { offset, limit: 200 }
        .deserialize(&mut serde_json::Deserializer::from_str(selected))
        .map_err(|_| "Not a JSON container or malformed/deep JSON".into())
}
pub fn kind(raw: &str) -> &'static str {
    match raw.trim_start().as_bytes().first() {
        Some(b'{') => "object",
        Some(b'[') => "array",
        Some(b'"') => "string",
        Some(b't' | b'f') => "boolean",
        Some(b'n') => "null",
        _ => "number",
    }
}

struct Named<'a> {
    name: &'a str,
}
impl<'de> Visitor<'de> for Named<'_> {
    type Value = Option<&'de RawValue>;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("attribute object")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut found = None;
        while let Some(key) = map.next_key::<String>()? {
            let value = map.next_value::<&'de RawValue>()?;
            if key == self.name && found.is_none() {
                found = Some(value);
            }
        }
        Ok(found)
    }
}
/// Explicit dot path, first duplicate wins, at most 16 path components; no DOM retained.
pub fn attribute(raw: &str, path: &str) -> Option<String> {
    if path.len() > 128 || path.split('.').count() > 16 {
        return None;
    }
    if !raw.trim_start().starts_with('{') {
        let mut found = None;
        crate::parse::decode::pairs(raw, |key, value| {
            if key == path && found.is_none() {
                found = Some(value.into_owned());
            }
        })
        .ok()?;
        return found;
    }
    let mut selected = raw;
    for name in path.split('.') {
        if name.is_empty() {
            return None;
        }
        let mut decoder = serde_json::Deserializer::from_str(selected);
        selected = de::Deserializer::deserialize_map(&mut decoder, Named { name })
            .ok()??
            .get();
    }
    match selected.as_bytes().first() {
        Some(b'"') => serde_json::from_str(selected).ok(),
        Some(b'{' | b'[') => None,
        _ => Some(selected.into()),
    }
}
