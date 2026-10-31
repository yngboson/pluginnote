//! note-core: 플랫폼 독립 코어.
//!
//! 객체 모델, 그리기 명령, 변환, 저장 포맷, SVG 내보내기를 담당한다.
//! 플러그인 실행(JS)은 알지 못하며 `PluginRuntime` 트레이트로만 접한다.

pub mod doc;
pub mod draw;
pub mod error;
pub mod svg;

pub use doc::{Document, Layer, Object, PluginRuntime, RenderedObject, Stroke, FORMAT_VERSION};
pub use draw::{Attachment, DrawCmd, PluginOutput, Pt, Readout, SnapPoint, Style};
pub use error::{CoreError, Result};
