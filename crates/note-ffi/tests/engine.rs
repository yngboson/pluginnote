use note_ffi::NoteEngine;
use serde_json::Value;

const MOHR: &str = include_str!("../../../plugins/mohr-circle/plugin.js");
const BEAM: &str = include_str!("../../../plugins/beam/plugin.js");
const SFD: &str = include_str!("../../../plugins/sfd-bmd/plugin.js");

fn frame(e: &NoteEngine) -> Value {
    serde_json::from_str(&e.render()).unwrap()
}

#[test]
fn ui_flow_add_edit_move_save_load() {
    let e = NoteEngine::new(vec![]);
    let m: Value = serde_json::from_str(&e.load_plugin(MOHR.into()).unwrap()).unwrap();
    assert_eq!(m["id"], "mohr-circle");
    e.add_object("m1".into(), "mohr-circle".into(), r#"{"sx":80,"sy":-40,"txy":30}"#.into(), 0.0, 0.0).unwrap();
    let f = frame(&e);
    assert_eq!(f["objects"][0]["ok"], true);
    assert_eq!(f["objects"][0]["draws"][0]["c"][0], 20.0);

    e.set_param("m1".into(), "sx".into(), "0".into()).unwrap(); // 숫자 입력 → 중심 이동
    assert_eq!(frame(&e)["objects"][0]["draws"][0]["c"][0], -20.0);
    e.move_object("m1".into(), 10.0, 0.0).unwrap();
    assert_eq!(frame(&e)["objects"][0]["draws"][0]["c"][0], -10.0);

    let saved = e.document_json().unwrap();
    let e2 = NoteEngine::new(vec![]);
    e2.load_plugin(MOHR.into()).unwrap();
    e2.load_document(saved).unwrap();
    assert_eq!(frame(&e2)["objects"][0]["draws"][0]["c"][0], -10.0);
}

#[test]
fn one_failing_object_does_not_break_the_frame() {
    let e = NoteEngine::new(vec![]);
    e.load_plugin(MOHR.into()).unwrap();
    e.add_object("ok".into(), "mohr-circle".into(), "{}".into(), 0.0, 0.0).unwrap();
    e.add_object("bad".into(), "no-such-plugin".into(), "{}".into(), 0.0, 0.0).unwrap();
    let f = frame(&e);
    assert_eq!(f["objects"][0]["ok"], true);
    assert_eq!(f["objects"][1]["ok"], false);
    assert!(f["objects"][1]["error"].as_str().unwrap().contains("no-such-plugin"));
}

#[test]
fn refs_and_strokes_and_permissions() {
    let e = NoteEngine::new(vec![]);
    e.load_plugin(BEAM.into()).unwrap();
    e.load_plugin(SFD.into()).unwrap();
    e.add_object("b".into(), "beam".into(), "{}".into(), 0.0, 0.0).unwrap();
    e.add_object("d".into(), "sfd-bmd".into(), "{}".into(), 0.0, 0.0).unwrap();
    e.set_ref("d".into(), "beam".into(), "b".into()).unwrap();
    let f = frame(&e);
    assert_eq!(f["objects"][1]["ok"], true, "{f}");
    assert_eq!(f["objects"][1]["readouts"][3]["label"], "M_max");

    e.add_stroke(vec![0.0, 0.0, 0.5, 1.0, 1.0, 0.7], "#000".into(), 2.0).unwrap();
    assert_eq!(frame(&e)["stroke_count"], 1);
    assert!(e.add_stroke(vec![1.0, 2.0], "#000".into(), 1.0).is_err());

    let evil = "export default definePlugin({ id:'e', version:'1', apiVersion:'0.1', name:'e', description:'d', tags:['t'], license:'MIT', permissions:['network'], render(p,c){ c.line([0,0],[1,1]); } });";
    assert!(e.load_plugin(evil.into()).is_err());
    assert!(e.inspect_plugin(evil.into()).is_err());
    assert!(e.remove_object("b".into()));
    assert!(!e.remove_object("b".into()));
}

fn first_center(e: &NoteEngine) -> f64 {
    frame(e)["objects"][0]["draws"][0]["c"][0].as_f64().unwrap()
}

#[test]
fn undo_redo_objects_params_and_drag() {
    let e = NoteEngine::new(vec![]);
    e.load_plugin(MOHR.into()).unwrap();
    assert!(!e.can_undo());
    e.add_object("m".into(), "mohr-circle".into(), r#"{"sx":80,"sy":-40,"txy":30}"#.into(), 0.0, 0.0).unwrap();
    e.set_param("m".into(), "sx".into(), "0".into()).unwrap();
    assert_eq!(first_center(&e), -20.0);

    assert!(e.undo()); // 값 변경 취소
    assert_eq!(first_center(&e), 20.0);
    assert!(e.can_redo());
    assert!(e.redo());
    assert_eq!(first_center(&e), -20.0);

    // 드래그: 시작 때 checkpoint 한 번, 이동은 여러 번 → undo 한 번에 원위치
    e.checkpoint();
    for x in [1.0, 2.0, 3.0, 4.0] {
        e.move_object("m".into(), x, 0.0).unwrap();
    }
    assert_eq!(first_center(&e), -16.0);
    assert!(e.undo());
    assert_eq!(first_center(&e), -20.0);

    // 삭제와 추가도 되돌린다
    assert!(e.remove_object("m".into()));
    assert_eq!(frame(&e)["objects"].as_array().unwrap().len(), 0);
    assert!(e.undo());
    assert_eq!(frame(&e)["objects"].as_array().unwrap().len(), 1);

    // 새 변경은 redo 를 무효화한다
    assert!(e.undo()); // set_param 취소
    e.set_param("m".into(), "txy".into(), "10".into()).unwrap();
    assert!(!e.can_redo());
    assert!(!e.redo());
}

#[test]
fn undo_history_is_cleared_on_load_and_bounded() {
    let e = NoteEngine::new(vec![]);
    e.load_plugin(MOHR.into()).unwrap();
    e.add_object("m".into(), "mohr-circle".into(), "{}".into(), 0.0, 0.0).unwrap();
    let saved = e.document_json().unwrap();
    for i in 0..150 {
        e.set_param("m".into(), "sx".into(), format!("{i}")).unwrap();
    }
    let mut n = 0;
    while e.undo() {
        n += 1;
    }
    assert_eq!(n, 100, "되돌리기 기록은 100개까지");
    e.load_document(saved).unwrap();
    assert!(!e.can_undo() && !e.can_redo());
}

#[test]
fn strokes_are_world_data_erasable_and_undoable() {
    let e = NoteEngine::new(vec![]);
    e.add_stroke(vec![0.0, 0.0, 0.5, 5.0, 5.0, 0.8], "#000".into(), 2.0).unwrap();
    e.add_stroke(vec![1.0, 1.0, 0.5, 2.0, 2.0, 0.5], "#f00".into(), 3.0).unwrap();
    let f = frame(&e);
    assert_eq!(f["strokes"].as_array().unwrap().len(), 2);
    assert_eq!(f["strokes"][0]["points"][1][0], 5.0);
    assert_eq!(f["strokes"][1]["style"]["stroke"], "#f00");

    assert!(e.remove_stroke(0)); // 지우개
    let f = frame(&e);
    assert_eq!(f["stroke_count"], 1);
    assert_eq!(f["strokes"][0]["style"]["stroke"], "#f00");
    assert!(!e.remove_stroke(5)); // 범위 밖

    assert!(e.undo()); // 지운 획 복원
    assert_eq!(frame(&e)["stroke_count"], 2);
    // 문서 저장에 획이 월드 좌표로 들어간다
    assert!(e.document_json().unwrap().contains("\"strokes\""));
}
