//! Shared serde helpers tolerating R jsonlite quirks (empty list `[]` where
//! an object/map was intended).

use serde::Deserialize;

#[derive(Deserialize)]
#[serde(untagged)]
enum MapOrEmpty<V> {
    Map(std::collections::HashMap<String, V>),
    Seq(Vec<serde_json::Value>),
}

pub fn map_or_empty<'de, D, V>(d: D) -> Result<std::collections::HashMap<String, V>, D::Error>
where
    D: serde::Deserializer<'de>,
    V: Deserialize<'de>,
{
    Ok(match MapOrEmpty::<V>::deserialize(d)? {
        MapOrEmpty::Map(m) => m,
        MapOrEmpty::Seq(v) => {
            if v.is_empty() {
                Default::default()
            } else {
                return Err(serde::de::Error::custom("expected map, got non-empty array"));
            }
        }
    })
}
