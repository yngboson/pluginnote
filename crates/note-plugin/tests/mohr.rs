use note_core::{Document, DrawCmd, Object};
use note_plugin::{JsHost, Limits, PluginError};
use serde_json::{json, Map, Value};
use std::time::Duration;

const MOHR: &str = include_str!("../../../plugins/mohr-circle/plugin.js");
const FBD: &str = include_str!("../../../plugins/fbd-arrow/plugin.js");

fn params(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}
fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}
fn circle_of(o: &note_core::PluginOutput) -> ([f64; 2], f64) {
    match &o.draws[0] {
        DrawCmd::Circle { c, r, .. } => (*c, *r),
        d => panic!("원이 아님: {d:?}"),
    }
}

#[test]
fn mohr_center_and_radius() {
    let mut h = JsHost::new();
    h.load(MOHR).unwrap();
    let o = h.render_plugin("mohr-circle", &params(json!({"sx": 80, "sy": -40, "txy": 30}))).unwrap();
    let (c, r) = circle_of(&o);
    near(c[0], 20.0);
    near(c[1], 0.0);
    near(r, (60f64 * 60.0 + 30.0 * 30.0).sqrt());
}

#[test]
fn mohr_has_vertical_and_horizontal_guides_through_center() {
    let mut h = JsHost::new();
    h.load(MOHR).unwrap();
    let o = h.render_plugin("mohr-circle", &params(json!({"sx": 80, "sy": -40, "txy": 30}))).unwrap();
    let names: Vec<_> = o.attachments.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["center-guide-v", "center-guide-h"]);
    match &o.attachments[0].draws[0] {
        DrawCmd::Line { from, to, .. } => {
            near(from[0], 20.0);
            near(to[0], 20.0); // 수직선
        }
        d => panic!("{d:?}"),
    }
    match &o.attachments[1].draws[0] {
        DrawCmd::Line { from, to, .. } => {
            near(from[1], 0.0);
            near(to[1], 0.0); // 수평선
        }
        d => panic!("{d:?}"),
    }
    assert_eq!(o.snaps[0].name, "center");
    near(o.snaps[0].at[0], 20.0);
}

#[test]
fn moving_object_moves_guides_and_changing_params_updates_them() {
    let mut h = JsHost::new();
    h.load(MOHR).unwrap();
    let mut doc = Document::default();
    doc.layers[0].objects.push(Object::new("m", "mohr-circle").with_param("sx", json!(80)).with_param("sy", json!(-40)).with_param("txy", json!(30)));

    let guide_x = |doc: &Document, h: &mut JsHost| match &doc.render(h).unwrap()[0].output.attachments[0].draws[0] {
        DrawCmd::Line { from, .. } => from[0],
        _ => unreachable!(),
    };
    near(guide_x(&doc, &mut h), 20.0);

    doc.object_mut("m").unwrap().origin = [100.0, 5.0]; // 이동
    near(guide_x(&doc, &mut h), 120.0);

    doc.object_mut("m").unwrap().params.insert("sx".into(), json!(0)); // 값 변경 → 중심 -20
    near(guide_x(&doc, &mut h), 80.0);
}

#[test]
fn document_roundtrip_stores_values_only() {
    let mut doc = Document::default();
    doc.layers[0].objects.push(Object::new("m", "mohr-circle").at(3.0, 4.0).with_param("sx", json!(10)));
    let s = doc.to_json().unwrap();
    assert!(!s.contains("draws"), "파생 그림은 저장하지 않는다");
    assert_eq!(Document::from_json(&s).unwrap(), doc);
}

#[test]
fn newer_format_is_rejected() {
    let s = r#"{"format":"pluginnote","format_version":999,"layers":[]}"#;
    assert!(Document::from_json(s).is_err());
}

#[test]
fn builtin_shapes_need_no_plugin() {
    let mut h = JsHost::new();
    let mut doc = Document::default();
    doc.layers[0].objects.push(Object::new("c", "circle").with_param("r", json!(2)));
    assert_eq!(doc.render(&mut h).unwrap().len(), 1);
}

#[test]
fn unknown_type_errors() {
    let mut h = JsHost::new();
    let mut doc = Document::default();
    doc.layers[0].objects.push(Object::new("x", "nope"));
    assert!(doc.render(&mut h).is_err());
}

#[test]
fn second_plugin_type_works_with_same_api() {
    let mut h = JsHost::new();
    h.load(FBD).unwrap();
    let o = h.render_plugin("fbd-arrow", &params(json!({"magnitude": 20, "angle": 90, "scale": 0.5}))).unwrap();
    near(o.readouts[0].value.as_f64().unwrap(), 0.0 + 20.0 * (90f64.to_radians()).cos());
    near(o.readouts[1].value.as_f64().unwrap(), 20.0);
}

#[test]
fn param_validation() {
    let mut h = JsHost::new();
    h.load(FBD).unwrap();
    assert!(matches!(h.render_plugin("fbd-arrow", &params(json!({"magnitude": -1}))), Err(PluginError::Params(_))));
    assert!(matches!(h.render_plugin("fbd-arrow", &params(json!({"typo": 1}))), Err(PluginError::Params(_))));
}

fn plugin_src(body: &str, perms: &str) -> String {
    format!("export default definePlugin({{ id:'t', version:'1', apiVersion:'0.1', permissions:{perms}, render(p, ctx) {{ {body} }} }});")
}

#[test]
fn undeclared_permission_is_denied_by_default() {
    let mut h = JsHost::new();
    let e = h.load(&plugin_src("", "['network']")).unwrap_err();
    assert!(matches!(e, PluginError::Permission(_)));
    // 호스트가 명시적으로 허용했을 때만 통과
    assert!(JsHost::new().grant("network").load(&plugin_src("", "['network']")).is_ok());
}

#[test]
fn sandbox_has_no_io_globals() {
    let mut h = JsHost::new();
    h.load(&plugin_src("ctx.text([0,0], [typeof fetch, typeof require, typeof process, typeof std, typeof os, typeof setTimeout, typeof XMLHttpRequest].join(','))", "[]")).unwrap();
    let o = h.render_plugin("t", &Map::new()).unwrap();
    match &o.draws[0] {
        DrawCmd::Text { text, .. } => assert_eq!(text, "undefined,undefined,undefined,undefined,undefined,undefined,undefined"),
        d => panic!("{d:?}"),
    }
}

#[test]
fn infinite_loop_is_interrupted() {
    let mut h = JsHost::new().with_limits(Limits { time_per_call: Duration::from_millis(50), ..Limits::default() });
    h.load(&plugin_src("while(true){}", "[]")).unwrap();
    assert!(matches!(h.render_plugin("t", &Map::new()), Err(PluginError::Timeout(_))));
    // 중단 후에도 호스트는 살아 있다
    h.load(MOHR).unwrap();
    assert!(h.render_plugin("mohr-circle", &Map::new()).is_ok());
}

#[test]
fn memory_bomb_fails_without_crashing_host() {
    let mut h = JsHost::new().with_limits(Limits { memory_bytes: 4 << 20, time_per_call: Duration::from_secs(2), ..Limits::default() });
    h.load(&plugin_src("const a=[]; for(;;) a.push(new Array(10000).fill(1));", "[]")).unwrap();
    assert!(h.render_plugin("t", &Map::new()).is_err());
}

#[test]
fn incompatible_api_major_is_rejected() {
    let src = "export default definePlugin({ id:'t', version:'1', apiVersion:'9.0', render(){} });";
    assert!(matches!(JsHost::new().load(src), Err(PluginError::ApiVersion { .. })));
}

#[test]
fn bad_point_gives_render_error_not_panic() {
    let mut h = JsHost::new();
    h.load(&plugin_src("ctx.line([NaN,0],[1,1])", "[]")).unwrap();
    assert!(matches!(h.render_plugin("t", &Map::new()), Err(PluginError::Render(_))));
}
