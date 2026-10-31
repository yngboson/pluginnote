//! 그리기 명령. Rust 코어는 이것만 만들고, 실제 그리기는 플랫폼 네이티브가 한다.
//! 좌표계: 월드 좌표, y 위쪽이 양수(수학 좌표). 각도는 라디안, 반시계 양수.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type Pt = [f64; 2];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Style {
    #[serde(default = "default_stroke")]
    pub stroke: String,
    #[serde(default = "default_width")]
    pub width: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dash: Option<Vec<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
}

fn default_stroke() -> String {
    "#111111".into()
}
fn default_width() -> f64 {
    1.5
}

impl Default for Style {
    fn default() -> Self {
        Style { stroke: default_stroke(), width: default_width(), dash: None, fill: None }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DrawCmd {
    Line { from: Pt, to: Pt, #[serde(default)] style: Style },
    Circle { c: Pt, r: f64, #[serde(default)] style: Style },
    /// c 중심, 반지름 r, start→end 반시계 방향 호
    Arc { c: Pt, r: f64, start: f64, end: f64, #[serde(default)] style: Style },
    Polyline { points: Vec<Pt>, #[serde(default)] style: Style },
    Arrow { from: Pt, to: Pt, #[serde(default)] style: Style },
    Text { at: Pt, text: String, #[serde(default = "default_text_size")] size: f64, #[serde(default)] style: Style },
}

fn default_text_size() -> f64 {
    12.0
}

/// 객체에 붙는 부속 요소(보조선 등). 이름이 있어 UI에서 토글/강조할 수 있다.
/// 저장되지 않고 params로부터 매번 파생된다 → 객체를 옮기거나 값을 바꾸면 함께 갱신된다.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Attachment {
    pub name: String,
    pub draws: Vec<DrawCmd>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapPoint {
    pub name: String,
    pub at: Pt,
}

/// 계산 결과 표시. 값과 함께 계산식을 노출해 AI 생성 플러그인의 오류를 사용자가 검증할 수 있게 한다.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Readout {
    pub label: String,
    pub value: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formula: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PluginOutput {
    #[serde(default)]
    pub draws: Vec<DrawCmd>,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    #[serde(default)]
    pub snaps: Vec<SnapPoint>,
    #[serde(default)]
    pub readouts: Vec<Readout>,
}

/// 객체의 로컬 → 월드 변환 (이동 + 균등 확대 + 회전).
#[derive(Debug, Clone, Copy)]
pub struct Xform {
    pub origin: Pt,
    pub scale: f64,
    pub rotation: f64,
}

impl Xform {
    pub fn pt(&self, p: Pt) -> Pt {
        let (s, c) = self.rotation.sin_cos();
        let (x, y) = (p[0] * self.scale, p[1] * self.scale);
        [self.origin[0] + x * c - y * s, self.origin[1] + x * s + y * c]
    }

    pub fn cmd(&self, d: &DrawCmd) -> DrawCmd {
        let sc = |st: &Style| {
            let mut s = st.clone();
            s.width *= 1.0; // 선 굵기는 화면 기준이라 확대하지 않는다
            s
        };
        match d {
            DrawCmd::Line { from, to, style } => DrawCmd::Line { from: self.pt(*from), to: self.pt(*to), style: sc(style) },
            DrawCmd::Circle { c, r, style } => DrawCmd::Circle { c: self.pt(*c), r: r * self.scale, style: sc(style) },
            DrawCmd::Arc { c, r, start, end, style } => DrawCmd::Arc {
                c: self.pt(*c),
                r: r * self.scale,
                start: start + self.rotation,
                end: end + self.rotation,
                style: sc(style),
            },
            DrawCmd::Polyline { points, style } => DrawCmd::Polyline { points: points.iter().map(|p| self.pt(*p)).collect(), style: sc(style) },
            DrawCmd::Arrow { from, to, style } => DrawCmd::Arrow { from: self.pt(*from), to: self.pt(*to), style: sc(style) },
            DrawCmd::Text { at, text, size, style } => DrawCmd::Text { at: self.pt(*at), text: text.clone(), size: *size, style: sc(style) },
        }
    }

    pub fn output(&self, o: &PluginOutput) -> PluginOutput {
        PluginOutput {
            draws: o.draws.iter().map(|d| self.cmd(d)).collect(),
            attachments: o
                .attachments
                .iter()
                .map(|a| Attachment { name: a.name.clone(), draws: a.draws.iter().map(|d| self.cmd(d)).collect() })
                .collect(),
            snaps: o.snaps.iter().map(|s| SnapPoint { name: s.name.clone(), at: self.pt(s.at) }).collect(),
            readouts: o.readouts.clone(),
        }
    }
}
