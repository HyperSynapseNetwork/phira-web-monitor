use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct ExtraJson {
    pub hitsounds: Option<HashMap<String, String>>,
}

pub fn parse_extra(source: &str) -> Result<ExtraJson> {
    let extra: ExtraJson = serde_json::from_str(source)?;
    Ok(extra)
}
