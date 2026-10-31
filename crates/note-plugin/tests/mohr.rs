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
    let o = h.render_plugin("mohr-circle", &params(json!({"sx": 80, "sy": -40, "txy": 30})), &Map::new()).unwrap();
    let (c, r) = circle_of(&o);
    near(c[0], 20.0);
    near(c[1], 0.0);
    near(r, (60f64 * 60.0 + 30.0 * 30.0).sqrt());
}

#[test]
fn mohr_has_vertical_and_horizontal_guides_through_center() {
    let mut h = JsHost::new();
    h.load(MOHR).unwrap();
    let o = h.render_plugin("mohr-circle", &params(json!({"sx": 80, "sy": -40, "txy": 30})), &Map::new()).unwrap();
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
    let o = h.render_plugin("fbd-arrow", &params(json!({"magnitude": 20, "angle": 90, "scale": 0.5})), &Map::new()).unwrap();
    near(o.readouts[0].value.as_f64().unwrap(), 0.0 + 20.0 * (90f64.to_radians()).cos());
    near(o.readouts[1].value.as_f64().unwrap(), 20.0);
}

#[test]
fn param_validation() {
    let mut h = JsHost::new();
    h.load(FBD).unwrap();
    assert!(matches!(h.render_plugin("fbd-arrow", &params(json!({"magnitude": -1})), &Map::new()), Err(PluginError::Params(_))));
    assert!(matches!(h.render_plugin("fbd-arrow", &params(json!({"typo": 1})), &Map::new()), Err(PluginError::Params(_))));
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
    let o = h.render_plugin("t", &Map::new(), &Map::new()).unwrap();
    match &o.draws[0] {
        DrawCmd::Text { text, .. } => assert_eq!(text, "undefined,undefined,undefined,undefined,undefined,undefined,undefined"),
        d => panic!("{d:?}"),
    }
}

#[test]
fn infinite_loop_is_interrupted() {
    let mut h = JsHost::new().with_limits(Limits { time_per_call: Duration::from_millis(50), ..Limits::default() });
    h.load(&plugin_src("while(true){}", "[]")).unwrap();
    assert!(matches!(h.render_plugin("t", &Map::new(), &Map::new()), Err(PluginError::Timeout(_))));
    // 중단 후에도 호스트는 살아 있다
    h.load(MOHR).unwrap();
    assert!(h.render_plugin("mohr-circle", &Map::new(), &Map::new()).is_ok());
}

#[test]
fn memory_bomb_fails_without_crashing_host() {
    let mut h = JsHost::new().with_limits(Limits { memory_bytes: 4 << 20, time_per_call: Duration::from_secs(2), ..Limits::default() });
    h.load(&plugin_src("const a=[]; for(;;) a.push(new Array(10000).fill(1));", "[]")).unwrap();
    assert!(h.render_plugin("t", &Map::new(), &Map::new()).is_err());
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
    assert!(matches!(h.render_plugin("t", &Map::new(), &Map::new()), Err(PluginError::Render(_))));
}

#[test]
fn worker_host_renders_off_thread_and_survives_timeout() {
    use note_plugin::WorkerHost;
    let mut w = WorkerHost::spawn(Limits { time_per_call: Duration::from_millis(50), ..Limits::default() }, &[]);
    w.load(MOHR).unwrap();
    w.load(&plugin_src("while(true){}", "[]")).unwrap();
    let main_thread = std::thread::current().id();
    let rx = w.render_async("mohr-circle", &Map::new(), &Map::new());
    // 호출 스레드는 막히지 않고 결과를 나중에 받는다
    let o = rx.recv().unwrap().unwrap();
    assert_eq!(o.attachments.len(), 2);
    assert_eq!(main_thread, std::thread::current().id());
    assert!(w.render_async("t", &Map::new(), &Map::new()).recv().unwrap().is_err());
    assert!(w.render_async("mohr-circle", &Map::new(), &Map::new()).recv().unwrap().is_ok());
}

const BEAM: &str = include_str!("../../../plugins/beam/plugin.js");
const SFD: &str = include_str!("../../../plugins/sfd-bmd/plugin.js");

fn readout(r: &note_core::RenderedObject, label: &str) -> f64 {
    r.output.readouts.iter().find(|x| x.label == label).unwrap_or_else(|| panic!("{label} 없음")).value.as_f64().unwrap()
}

fn beam_doc(loads: Value) -> Document {
    let mut d = Document::default();
    d.layers[0].objects.push(Object::new("b", "beam").with_param("L", json!(10)).with_param("loads", loads));
    d.layers[0].objects.push(Object::new("d", "sfd-bmd").at(0.0, 0.0).with_ref("beam", "b"));
    d
}

#[test]
fn sfd_bmd_point_load_matches_closed_form() {
    let mut h = JsHost::new();
    h.load(BEAM).unwrap();
    h.load(SFD).unwrap();
    let r = beam_doc(json!([{"kind":"point","x":5,"P":20}])).render(&mut h).unwrap();
    let d = &r[1];
    near(readout(d, "R_A"), 10.0);
    near(readout(d, "R_B"), 10.0);
    near(readout(d, "|V|max"), 10.0);
    near(readout(d, "M_max"), 50.0); // PL/4
    near(readout(d, "x @ M_max"), 5.0);
}

#[test]
fn sfd_bmd_udl_matches_closed_form() {
    let mut h = JsHost::new();
    h.load(BEAM).unwrap();
    h.load(SFD).unwrap();
    let r = beam_doc(json!([{"kind":"udl","a":0,"b":10,"w":2}])).render(&mut h).unwrap();
    near(readout(&r[1], "R_A"), 10.0);
    near(readout(&r[1], "M_max"), 25.0); // wL²/8
    near(readout(&r[1], "x @ M_max"), 5.0);
}

#[test]
fn sfd_bmd_unsymmetric_load_reactions() {
    let mut h = JsHost::new();
    h.load(BEAM).unwrap();
    h.load(SFD).unwrap();
    let r = beam_doc(json!([{"kind":"point","x":2,"P":30}])).render(&mut h).unwrap();
    near(readout(&r[1], "R_B"), 6.0); // 30*2/10
    near(readout(&r[1], "R_A"), 24.0);
    near(readout(&r[1], "M_max"), 48.0); // Pab/L = 30*2*8/10
}

#[test]
fn diagram_follows_beam_when_beam_changes() {
    let mut h = JsHost::new();
    h.load(BEAM).unwrap();
    h.load(SFD).unwrap();
    let mut doc = beam_doc(json!([{"kind":"point","x":5,"P":20}]));
    assert_eq!(readout(&doc.render(&mut h).unwrap()[1], "M_max"), 50.0);
    doc.object_mut("b").unwrap().params.insert("loads".into(), json!([{"kind":"point","x":5,"P":40}]));
    near(readout(&doc.render(&mut h).unwrap()[1], "M_max"), 100.0);
}

#[test]
fn diagram_aligns_with_beam_position() {
    let mut h = JsHost::new();
    h.load(BEAM).unwrap();
    h.load(SFD).unwrap();
    let mut doc = beam_doc(json!([{"kind":"point","x":5,"P":20}]));
    doc.object_mut("b").unwrap().origin = [7.0, 3.0]; // 보를 옮기면 offset 이 바뀌어 선도 x 가 보와 정렬
    let r = doc.render(&mut h).unwrap();
    // 선도(원점 0,0)의 축 첫 선은 보 시작점 x=7 월드에서 시작해야 한다
    match &r[1].output.attachments[0].draws[0] {
        DrawCmd::Line { from, .. } => near(from[0], 7.0),
        d => panic!("{d:?}"),
    }
    near(r[1].output.snaps[0].at[0], 7.0 + 5.0);
}

#[test]
fn missing_or_wrong_ref_is_error() {
    let mut h = JsHost::new();
    h.load(BEAM).unwrap();
    h.load(SFD).unwrap();
    let mut d = Document::default();
    d.layers[0].objects.push(Object::new("d", "sfd-bmd"));
    assert!(d.render(&mut h).is_err()); // 연결 없음
    let mut d = Document::default();
    d.layers[0].objects.push(Object::new("c", "circle"));
    d.layers[0].objects.push(Object::new("d", "sfd-bmd").with_ref("beam", "c"));
    assert!(d.render(&mut h).is_err()); // type 불일치
    let mut d = Document::default();
    d.layers[0].objects.push(Object::new("d", "sfd-bmd").with_ref("beam", "ghost"));
    assert!(d.render(&mut h).is_err()); // 대상 없음
}

const SS: &str = include_str!("../../../plugins/stress-strain/plugin.json");

#[test]
fn declarative_stress_strain_geometry() {
    let mut h = JsHost::new();
    h.load(SS).unwrap();
    let o = h.render_plugin("stress-strain", &Map::new(), &Map::new()).unwrap();
    match &o.draws[0] {
        DrawCmd::Polyline { points, .. } => {
            assert_eq!(points[0], [0.0, 0.0]);
            near(points[1][0], 250.0 / 200000.0 * 8.0 * 1000.0); // 항복점 x = εy * kel * xscale (도식용 과장)
            near(points[1][1], 250.0);
            near(points.iter().map(|p| p[1]).fold(0.0, f64::max), 400.0); // 최대 응력 = σu
        }
        d => panic!("{d:?}"),
    }
    let yld = o.snaps.iter().find(|s| s.name == "yield").unwrap();
    near(yld.at[1], 250.0);
    // 0.2% offset 선은 표시된 탄성 직선과 평행
    let slope = 250.0 / (250.0 / 200000.0 * 8.0 * 1000.0);
    match &o.attachments.iter().find(|a| a.name == "offset-line").unwrap().draws[0] {
        DrawCmd::Line { from, to, .. } => near((to[1] - from[1]) / (to[0] - from[0]), slope),
        d => panic!("{d:?}"),
    }
}

#[test]
fn declarative_needs_no_js_and_rejects_bad_expressions() {
    let bad = r#"{"id":"x","version":"1","apiVersion":"0.1","draw":[{"op":"line","from":["1","0"],"to":["system(1)","0"]}]}"#;
    let mut h = JsHost::new();
    h.load(bad).unwrap();
    assert!(matches!(h.render_plugin("x", &Map::new(), &Map::new()), Err(PluginError::Render(_))));
    let div0 = r#"{"id":"y","version":"1","apiVersion":"0.1","draw":[{"op":"line","from":["1/0","0"],"to":["1","0"]}]}"#;
    h.load(div0).unwrap();
    assert!(h.render_plugin("y", &Map::new(), &Map::new()).is_err());
    let undefined = r#"{"id":"z","version":"1","apiVersion":"0.1","draw":[{"op":"circle","cx":"0","cy":"0","r":"nope"}]}"#;
    h.load(undefined).unwrap();
    assert!(h.render_plugin("z", &Map::new(), &Map::new()).is_err());
}

#[test]
fn declarative_permission_and_param_checks_apply() {
    let src = r#"{"id":"p","version":"1","apiVersion":"0.1","permissions":["network"],"draw":[]}"#;
    assert!(matches!(JsHost::new().load(src), Err(PluginError::Permission(_))));
    let mut h = JsHost::new();
    h.load(SS).unwrap();
    assert!(h.render_plugin("stress-strain", &params(json!({"E": 0})), &Map::new()).is_err()); // min 위반
}

#[test]
fn ref_target_params_get_target_plugin_defaults() {
    let mut h = JsHost::new();
    h.load(BEAM).unwrap();
    h.load(SFD).unwrap();
    let mut d = Document::default();
    d.layers[0].objects.push(Object::new("b", "beam")); // params 비어 있음 → L=10, 중앙 20 기본
    d.layers[0].objects.push(Object::new("d", "sfd-bmd").with_ref("beam", "b"));
    near(readout(&d.render(&mut h).unwrap()[1], "M_max"), 50.0);
}

const MOHR_P: &str = include_str!("../../../plugins/mohr-principal/plugin.js");

#[test]
fn mohr_principal_extension_via_refs_without_touching_preset() {
    let mut h = JsHost::new();
    h.load(MOHR).unwrap();
    h.load(MOHR_P).unwrap();
    let mut d = Document::default();
    d.layers[0].objects.push(Object::new("m", "mohr-circle").with_param("sx", json!(80)).with_param("sy", json!(-40)).with_param("txy", json!(30)));
    d.layers[0].objects.push(Object::new("p", "mohr-principal").with_ref("circle", "m"));
    let r = d.render(&mut h).unwrap();
    let rr = (60f64 * 60.0 + 30.0 * 30.0).sqrt();
    near(readout(&r[1], "σ1"), 20.0 + rr);
    near(readout(&r[1], "σ2"), 20.0 - rr);
    near(readout(&r[1], "τmax"), rr);
    near(readout(&r[1], "θp (deg)"), 0.5 * (30f64).atan2(60.0).to_degrees());

    // 주응력은 원 위에 있어야 한다: σ1 스냅점은 원의 오른쪽 끝
    let s1 = r[1].output.snaps.iter().find(|s| s.name == "sigma1").unwrap();
    let (c, rad) = circle_of(&r[0].output);
    near(s1.at[0], c[0] + rad);

    // 원 객체를 옮기면 주석이 함께 따라간다
    d.object_mut("m").unwrap().origin = [100.0, 0.0];
    let r2 = d.render(&mut h).unwrap();
    near(r2[1].output.snaps.iter().find(|s| s.name == "sigma1").unwrap().at[0], 100.0 + c[0] + rad);
}

#[test]
fn mohr_principal_pure_shear_gives_45_degrees() {
    let mut h = JsHost::new();
    h.load(MOHR).unwrap();
    h.load(MOHR_P).unwrap();
    let mut d = Document::default();
    d.layers[0].objects.push(Object::new("m", "mohr-circle").with_param("sx", json!(0)).with_param("sy", json!(0)).with_param("txy", json!(50)));
    d.layers[0].objects.push(Object::new("p", "mohr-principal").with_ref("circle", "m"));
    let r = d.render(&mut h).unwrap();
    near(readout(&r[1], "θp (deg)"), 45.0);
    near(readout(&r[1], "σ1"), 50.0);
}
