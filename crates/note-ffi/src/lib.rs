//! Swift/Kotlin 에 노출되는 얇은 인터페이스.
//!
//! 원칙: 동작 규칙은 전부 Rust 에 있고, UI 는 이 객체를 호출해 "그릴 것"을 받아 그리기만 한다.
//! 데이터는 JSON 문자열로 주고받아 두 플랫폼의 바인딩 표면을 작게 유지한다.
//! 잉크 렌더링(저지연)은 플랫폼 네이티브가 하며, 획 데이터만 `add_stroke` 로 넘겨 보관한다.

use note_core::{Document, Object, Stroke};
use note_plugin::{Limits, WorkerHost};
use serde_json::{json, Map, Value};
use std::sync::Mutex;

uniffi::setup_scaffolding!();

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum NoteError {
    #[error("{msg}")]
    Failed { msg: String },
}

fn fail<E: std::fmt::Display>(e: E) -> NoteError {
    NoteError::Failed { msg: e.to_string() }
}

struct Inner {
    doc: Document,
    host: WorkerHost,
}

#[derive(uniffi::Object)]
pub struct NoteEngine {
    inner: Mutex<Inner>,
}

#[uniffi::export]
impl NoteEngine {
    /// `granted_permissions`: 사용자가 허용한 권한. 일반적으로 빈 목록.
    #[uniffi::constructor]
    pub fn new(granted_permissions: Vec<String>) -> Self {
        let g: Vec<&str> = granted_permissions.iter().map(String::as_str).collect();
        NoteEngine { inner: Mutex::new(Inner { doc: Document::default(), host: WorkerHost::spawn(Limits::default(), &g) }) }
    }

    /// 플러그인 소스(JS 또는 선언형 JSON)를 설치. manifest JSON 반환 → UI 가 권한/파라미터 편집 UI 구성에 사용.
    pub fn load_plugin(&self, source: String) -> Result<String, NoteError> {
        let m = self.inner.lock().unwrap().host.load(&source).map_err(fail)?;
        serde_json::to_string(&m).map_err(fail)
    }

    /// 설치 전 권한 확인용: 소스를 격리 검사하고 manifest 를 돌려준다(설치하지 않음).
    pub fn inspect_plugin(&self, source: String) -> Result<String, NoteError> {
        let (m, _, _) = note_registry::check_plugin(&source).map_err(fail)?;
        serde_json::to_string(&m).map_err(fail)
    }

    pub fn load_document(&self, json: String) -> Result<(), NoteError> {
        self.inner.lock().unwrap().doc = Document::from_json(&json).map_err(fail)?;
        Ok(())
    }

    pub fn document_json(&self) -> Result<String, NoteError> {
        self.inner.lock().unwrap().doc.to_json().map_err(fail)
    }

    pub fn add_object(&self, id: String, ty: String, params_json: String, x: f64, y: f64) -> Result<(), NoteError> {
        let params: Map<String, Value> = serde_json::from_str(&params_json).map_err(fail)?;
        let mut g = self.inner.lock().unwrap();
        if g.doc.object(&id).is_some() {
            return Err(fail(format!("이미 있는 id: {id}")));
        }
        let mut o = Object::new(&id, &ty).at(x, y);
        o.params = params;
        g.doc.layers[0].objects.push(o);
        Ok(())
    }

    pub fn set_ref(&self, id: String, name: String, target: String) -> Result<(), NoteError> {
        let mut g = self.inner.lock().unwrap();
        g.doc.object_mut(&id).ok_or_else(|| fail(format!("객체 없음: {id}")))?.refs.insert(name, Value::String(target));
        Ok(())
    }

    /// 속성 편집(숫자 입력, 슬라이더 등)의 단일 진입점. value_json 은 JSON 값.
    pub fn set_param(&self, id: String, key: String, value_json: String) -> Result<(), NoteError> {
        let v: Value = serde_json::from_str(&value_json).map_err(fail)?;
        let mut g = self.inner.lock().unwrap();
        g.doc.object_mut(&id).ok_or_else(|| fail(format!("객체 없음: {id}")))?.params.insert(key, v);
        Ok(())
    }

    pub fn move_object(&self, id: String, x: f64, y: f64) -> Result<(), NoteError> {
        let mut g = self.inner.lock().unwrap();
        g.doc.object_mut(&id).ok_or_else(|| fail(format!("객체 없음: {id}")))?.origin = [x, y];
        Ok(())
    }

    pub fn remove_object(&self, id: String) -> bool {
        let mut g = self.inner.lock().unwrap();
        let mut removed = false;
        for l in &mut g.doc.layers {
            let n = l.objects.len();
            l.objects.retain(|o| o.id != id);
            removed |= l.objects.len() != n;
        }
        removed
    }

    /// 잉크 획 보관. points 는 [x, y, 필압] 반복의 평탄 배열.
    pub fn add_stroke(&self, flat_xyp: Vec<f32>, stroke_color: String, width: f64) -> Result<(), NoteError> {
        if flat_xyp.len() % 3 != 0 {
            return Err(fail("points 길이는 3의 배수여야 함"));
        }
        let points = flat_xyp.chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
        let style = note_core::Style { stroke: stroke_color, width, dash: None, fill: None };
        self.inner.lock().unwrap().doc.layers[0].strokes.push(Stroke { points, style });
        Ok(())
    }

    /// 한 프레임 분량의 그리기 명령(JSON). 객체별로 오류를 분리해, 한 플러그인이 실패해도 나머지는 그린다.
    /// { objects: [{id, ok, error?, draws, attachments, snaps, readouts}] }
    pub fn render(&self) -> String {
        let mut g = self.inner.lock().unwrap();
        let Inner { doc, host } = &mut *g;
        let mut out = vec![];
        for obj in doc.layers.iter().flat_map(|l| l.objects.iter()) {
            match doc.render_one(obj, host) {
                Ok(me) => out.push(json!({"id": obj.id, "ok": true, "draws": me.output.draws, "attachments": me.output.attachments, "snaps": me.output.snaps, "readouts": me.output.readouts})),
                Err(e) => out.push(json!({"id": obj.id, "ok": false, "error": e.to_string()})),
            }
        }
        let strokes: Vec<_> = doc.layers.iter().flat_map(|l| l.strokes.iter()).collect();
        json!({"objects": out, "stroke_count": strokes.len()}).to_string()
    }

    pub fn export_svg(&self) -> Result<String, NoteError> {
        let mut g = self.inner.lock().unwrap();
        let Inner { doc, host } = &mut *g;
        let r = doc.render(host).map_err(fail)?;
        Ok(note_core::svg::to_svg(&r))
    }
}

#[uniffi::export]
pub fn core_version() -> String {
    env!("CARGO_PKG_VERSION").into()
}

#[uniffi::export]
pub fn plugin_api_version() -> String {
    note_plugin::API_VERSION.into()
}
