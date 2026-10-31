//! 문서 모델과 저장 포맷 (공개 규격 후보). 파일은 UTF-8 JSON.
//!
//! 저장되는 것은 "값"뿐이다: 객체는 `id/type/params`와 배치(origin/scale/rotation)만 갖고,
//! 그림과 부속 요소는 렌더 때마다 파생한다.

use crate::draw::{DrawCmd, PluginOutput, Pt, Style, Xform};
use crate::error::{CoreError, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const FORMAT_VERSION: u32 = 1;
const FORMAT_NAME: &str = "pluginnote";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub format: String,
    pub format_version: u32,
    #[serde(default)]
    pub layers: Vec<Layer>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub strokes: Vec<Stroke>,
    #[serde(default)]
    pub objects: Vec<Object>,
}

/// 잉크 획. 샘플은 [x, y, 필압]. 렌더링/저지연 처리는 플랫폼 네이티브 몫이고, 코어는 보관만 한다.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub points: Vec<[f32; 3]>,
    #[serde(default)]
    pub style: Style,
}

fn one() -> f64 {
    1.0
}
fn is_zero(v: &f64) -> bool {
    *v == 0.0
}
fn is_one(v: &f64) -> bool {
    *v == 1.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Object {
    pub id: String,
    /// 내장: "line" | "circle" | "rect". 그 외는 플러그인 id.
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default)]
    pub params: Map<String, Value>,
    #[serde(default)]
    pub origin: Pt,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub scale: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rotation: f64,
    /// 다른 객체에 대한 연결: 이름 → 객체 id. 대상이 바뀌면 이 객체가 재계산된다(예: SFD/BMD → 보).
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub refs: Map<String, Value>,
}

impl Object {
    pub fn new(id: &str, ty: &str) -> Self {
        Object { id: id.into(), ty: ty.into(), params: Map::new(), origin: [0.0, 0.0], scale: 1.0, rotation: 0.0, refs: Map::new() }
    }
    pub fn with_param(mut self, k: &str, v: Value) -> Self {
        self.params.insert(k.into(), v);
        self
    }
    pub fn with_ref(mut self, name: &str, id: &str) -> Self {
        self.refs.insert(name.into(), Value::String(id.into()));
        self
    }
    pub fn at(mut self, x: f64, y: f64) -> Self {
        self.origin = [x, y];
        self
    }
    pub fn xform(&self) -> Xform {
        Xform { origin: self.origin, scale: self.scale, rotation: self.rotation }
    }
}

/// 코어가 플러그인 실행기를 알지 않도록 하는 경계. note-plugin 이 구현한다.
pub trait PluginRuntime {
    fn has(&self, ty: &str) -> bool;
    /// params 는 누락 값이 기본값으로 채워지고 검증된 상태로 렌더된다. 출력은 객체 로컬 좌표.
    /// `refs`: 연결 이름 → {id, type, params, offset}. offset 은 대상 원점 − 자기 원점(자기 로컬 좌표 기준 위치 정렬용).
    fn render(&mut self, ty: &str, params: &Map<String, Value>, refs: &Map<String, Value>) -> Result<PluginOutput>;
}

#[derive(Debug, Clone)]
pub struct RenderedObject {
    pub id: String,
    /// 월드 좌표로 변환된 결과
    pub output: PluginOutput,
}

impl Default for Document {
    fn default() -> Self {
        Document { format: FORMAT_NAME.into(), format_version: FORMAT_VERSION, layers: vec![Layer { id: "L1".into(), name: "Layer 1".into(), ..Default::default() }] }
    }
}

impl Document {
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn from_json(s: &str) -> Result<Self> {
        let d: Document = serde_json::from_str(s)?;
        if d.format != FORMAT_NAME {
            return Err(CoreError::Invalid(format!("format 이 '{}' 이 아님: {}", FORMAT_NAME, d.format)));
        }
        if d.format_version > FORMAT_VERSION {
            return Err(CoreError::UnsupportedFormat(d.format_version, FORMAT_VERSION));
        }
        Ok(d)
    }

    pub fn object(&self, id: &str) -> Option<&Object> {
        self.layers.iter().flat_map(|l| l.objects.iter()).find(|o| o.id == id)
    }

    fn resolve_refs(&self, obj: &Object) -> Result<Map<String, Value>> {
        let mut out = Map::new();
        for (name, target) in &obj.refs {
            let tid = target.as_str().ok_or_else(|| CoreError::Invalid(format!("{}.refs.{name} 은 객체 id 문자열이어야 함", obj.id)))?;
            let t = self.object(tid).ok_or_else(|| CoreError::Invalid(format!("{}.refs.{name}: 객체 '{tid}' 없음", obj.id)))?;
            out.insert(
                name.clone(),
                serde_json::json!({
                    "id": t.id, "type": t.ty, "params": t.params,
                    "offset": [t.origin[0] - obj.origin[0], t.origin[1] - obj.origin[1]],
                }),
            );
        }
        Ok(out)
    }

    pub fn object_mut(&mut self, id: &str) -> Option<&mut Object> {
        self.layers.iter_mut().flat_map(|l| l.objects.iter_mut()).find(|o| o.id == id)
    }

    /// 모든 객체를 렌더한다. 잉크는 Polyline 으로 함께 반환하지 않는다(네이티브가 그린다).
    pub fn render(&self, rt: &mut dyn PluginRuntime) -> Result<Vec<RenderedObject>> {
        let mut out = Vec::new();
        for obj in self.layers.iter().flat_map(|l| l.objects.iter()) {
            let local = if let Some(o) = builtin(obj)? {
                o
            } else if rt.has(&obj.ty) {
                rt.render(&obj.ty, &obj.params, &self.resolve_refs(obj)?)?
            } else {
                return Err(CoreError::UnknownType(obj.ty.clone()));
            };
            out.push(RenderedObject { id: obj.id.clone(), output: obj.xform().output(&local) });
        }
        Ok(out)
    }
}

fn num(o: &Object, k: &str, default: f64) -> f64 {
    o.params.get(k).and_then(Value::as_f64).unwrap_or(default)
}

/// 개발사가 직접 책임지는 기본 도형.
fn builtin(o: &Object) -> Result<Option<PluginOutput>> {
    let style = Style::default();
    let draws = match o.ty.as_str() {
        "line" => vec![DrawCmd::Line { from: [0.0, 0.0], to: [num(o, "dx", 1.0), num(o, "dy", 0.0)], style }],
        "circle" => vec![DrawCmd::Circle { c: [0.0, 0.0], r: num(o, "r", 1.0), style }],
        "rect" => {
            let (w, h) = (num(o, "w", 1.0), num(o, "h", 1.0));
            vec![DrawCmd::Polyline { points: vec![[0.0, 0.0], [w, 0.0], [w, h], [0.0, h], [0.0, 0.0]], style }]
        }
        _ => return Ok(None),
    };
    Ok(Some(PluginOutput { draws, ..Default::default() }))
}
