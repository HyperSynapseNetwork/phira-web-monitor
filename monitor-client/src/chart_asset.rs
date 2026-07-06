use monitor_common::core::{Chart, ChartInfo};
use wasm_bindgen::{JsCast, prelude::*};

pub(crate) fn prepare_chart_order(chart: &mut Chart) {
    chart.order = (0..chart.lines.len()).collect();
    chart.order.sort_by_key(|&i| chart.lines[i].z_index);
}

pub(crate) fn decode_chart_bytes(bytes: &[u8]) -> Result<(ChartInfo, Chart), String> {
    use bincode::Options;

    let (info, mut chart): (ChartInfo, Chart) = bincode::options()
        .with_varint_encoding()
        .deserialize(bytes)
        .map_err(|e| format!("Failed to parse chart: {e}"))?;
    prepare_chart_order(&mut chart);
    Ok((info, chart))
}

pub(crate) async fn fetch_and_parse_chart(
    api_base: &str,
    id: impl std::fmt::Display,
) -> Result<(ChartInfo, Chart), JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    let resp_value = wasm_bindgen_futures::JsFuture::from(
        window.fetch_with_str(&format!("{api_base}/chart/{id}")),
    )
    .await?;
    let resp: web_sys::Response = resp_value.dyn_into()?;

    if !resp.ok() {
        return Err(JsValue::from_str(&format!(
            "Fetch failed: {}",
            resp.status_text()
        )));
    }

    let array_buffer = wasm_bindgen_futures::JsFuture::from(resp.array_buffer()?).await?;
    let uint8_array = js_sys::Uint8Array::new(&array_buffer);
    decode_chart_bytes(&uint8_array.to_vec()).map_err(|e| JsValue::from_str(&e))
}

pub(crate) fn file_map_from_js_object(
    files: js_sys::Object,
) -> Result<std::collections::HashMap<String, Vec<u8>>, JsValue> {
    let entries = js_sys::Object::entries(&files);
    let mut file_map = std::collections::HashMap::new();

    for i in 0..entries.length() {
        let entry = entries.get(i);
        let entry_array = js_sys::Array::from(&entry);
        let key = entry_array.get(0).as_string().ok_or("Invalid key")?;
        let value = entry_array.get(1);
        let uint8_array = js_sys::Uint8Array::new(&value);
        file_map.insert(key, uint8_array.to_vec());
    }

    Ok(file_map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use monitor_common::core::JudgeLine;

    fn encoded_chart_with_z_indexes(z_indexes: &[i32]) -> Vec<u8> {
        use bincode::Options;

        let info = ChartInfo::default();
        let chart = Chart {
            lines: z_indexes
                .iter()
                .map(|&z_index| JudgeLine {
                    z_index,
                    ..Default::default()
                })
                .collect(),
            order: vec![999],
            ..Default::default()
        };

        bincode::options()
            .with_varint_encoding()
            .serialize(&(info, chart))
            .unwrap()
    }

    #[test]
    fn prepare_chart_order_sorts_line_indexes_by_z_index() {
        let mut chart = Chart {
            lines: [30, -10, 10]
                .into_iter()
                .map(|z_index| JudgeLine {
                    z_index,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };

        prepare_chart_order(&mut chart);

        assert_eq!(chart.order, vec![1, 2, 0]);
    }

    #[test]
    fn decode_chart_bytes_restores_runtime_line_order() {
        let bytes = encoded_chart_with_z_indexes(&[2, 1, 3]);

        let (_info, chart) = decode_chart_bytes(&bytes).unwrap();

        assert_eq!(chart.order, vec![1, 0, 2]);
    }

    #[test]
    fn decode_chart_bytes_rejects_invalid_payload() {
        let err = match decode_chart_bytes(b"not a chart") {
            Ok(_) => panic!("invalid payload decoded successfully"),
            Err(err) => err,
        };

        assert!(err.contains("Failed to parse chart"));
    }
}
