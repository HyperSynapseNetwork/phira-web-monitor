use anyhow::Result;
use monitor_common::core::Chart;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct ExtraJson {
    pub hitsounds: Option<HashMap<String, String>>,
}

pub fn apply_extra(_chart: &mut Chart, source: &str) -> Result<()> {
    let _extra: ExtraJson = serde_json::from_str(source)?;
    // Hitsound loading from memory is not yet implemented in the proxy.
    // The monitor client will handle loading resources from the zip.
    log::info!("extra.json parsed but hitsound loading is deferred to client");
    Ok(())
}
