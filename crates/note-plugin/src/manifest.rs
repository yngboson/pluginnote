use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// 호스트가 제공하는 플러그인 API 버전. 주(major)가 다르면 로드하지 않는다.
pub const API_VERSION: &str = "0.1";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ParamSpec {
    pub default: Value,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    /// 속성 편집 UI 힌트: "number" | "slider". 키보드/드래그 입력 확장을 위한 자리.
    #[serde(default)]
    pub editor: Option<String>,
}

/// 다른 객체에 대한 연결 선언. `type` 이 일치하는 객체만 연결할 수 있다.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RefSpec {
    #[serde(rename = "type")]
    pub ty: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Example {
    #[serde(default)]
    pub name: Option<String>,
    pub params: Map<String, Value>,
    /// 연결이 필요한 플러그인의 검사용 가짜 대상: 이름 → {type, params, offset?}
    #[serde(default)]
    pub refs: Map<String, Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub id: String,
    pub version: String,
    pub api_version: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub params: BTreeMap<String, ParamSpec>,
    #[serde(default)]
    pub refs: BTreeMap<String, RefSpec>,
    #[serde(default)]
    pub examples: Vec<Example>,
}

impl Manifest {
    pub fn api_compatible(&self) -> bool {
        let major = |v: &str| v.split('.').next().map(str::to_owned);
        major(&self.api_version) == major(API_VERSION)
    }

    pub fn check_refs(&self, given: &Map<String, Value>) -> Result<(), String> {
        for (name, spec) in &self.refs {
            let r = given.get(name).ok_or_else(|| format!("연결 '{name}' 이(가) 필요함 (type={})", spec.ty))?;
            let ty = r.get("type").and_then(Value::as_str).unwrap_or("");
            if ty != spec.ty {
                return Err(format!("연결 '{name}' 은 type={} 이어야 하는데 {ty}", spec.ty));
            }
        }
        if let Some(k) = given.keys().find(|k| !self.refs.contains_key(*k)) {
            return Err(format!("선언되지 않은 연결: {k}"));
        }
        Ok(())
    }

    /// 누락된 값을 기본값으로 채우고, 숫자 범위를 검증한다. 모르는 키는 거부한다(오타 방지).
    pub fn resolve_params(&self, given: &Map<String, Value>) -> Result<Map<String, Value>, String> {
        for k in given.keys() {
            if !self.params.contains_key(k) {
                return Err(format!("알 수 없는 파라미터: {k}"));
            }
        }
        let mut out = Map::new();
        for (k, spec) in &self.params {
            let v = given.get(k).cloned().unwrap_or_else(|| spec.default.clone());
            if let Some(x) = v.as_f64() {
                if let Some(min) = spec.min {
                    if x < min {
                        return Err(format!("{k}={x} 는 최소값 {min} 미만"));
                    }
                }
                if let Some(max) = spec.max {
                    if x > max {
                        return Err(format!("{k}={x} 는 최대값 {max} 초과"));
                    }
                }
            } else if spec.default.is_number() {
                return Err(format!("{k} 는 숫자여야 합니다"));
            }
            out.insert(k.clone(), v);
        }
        Ok(out)
    }
}
