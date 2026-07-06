#[allow(dead_code)]
#[path = "../src/chart_asset.rs"]
mod chart_asset;
#[allow(dead_code)]
#[path = "../src/viewport.rs"]
mod viewport;

use monitor_common::core::{Chart, ChartInfo, JudgeLine};

fn encode_chart(z_indexes: &[i32]) -> Vec<u8> {
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
        ..Default::default()
    };

    bincode::options()
        .with_varint_encoding()
        .serialize(&(info, chart))
        .unwrap()
}

#[test]
fn decoded_chart_is_ready_for_render_order_iteration() {
    let (_info, chart) = chart_asset::decode_chart_bytes(&encode_chart(&[10, -5, 7])).unwrap();

    assert_eq!(chart.order, vec![1, 2, 0]);
}

#[test]
fn viewport_layout_matches_player_and_monitor_letterboxing_contract() {
    let layout = viewport::letterbox_viewport(1600, 1000, 16.0 / 9.0).unwrap();

    assert_eq!(layout.viewport.x, 0);
    assert_eq!(layout.viewport.y, 0);
    assert_eq!(layout.viewport.width, 1600);
    assert_eq!(layout.viewport.height, 1000);
    assert!((layout.aspect_ratio - 1.6).abs() < f32::EPSILON);
}
