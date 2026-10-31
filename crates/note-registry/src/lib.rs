//! 공유 레포(로컬 구현). 디렉터리 구조는 공개 Git 레포로 그대로 올릴 수 있다.
//!
//!   <root>/index.json
//!   <root>/plugins/<id>/<version>/{plugin.js|plugin.json, manifest.json, preview.svg}
//!
//! 업로드(publish)는 자동 검사를 통과해야 한다: 로드, 권한 선언, API 호환, 기본 예제 렌더, 중복 버전.
//! 검색은 이름/설명/태그/id 가중 키워드 매칭이다. 시맨틱(임베딩) 검색은 `Ranker` 로 교체 가능하게 뒀다.

use note_core::{svg::to_svg, Document, Object};
use note_plugin::{JsHost, Manifest};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("자동 검사 실패: {0}")]
    CheckFailed(String),
    #[error("이미 존재하는 버전: {0}@{1} (버전을 올려서 업로드)")]
    DuplicateVersion(String, String),
    #[error("찾을 수 없음: {0}")]
    NotFound(String),
}

type Result<T> = std::result::Result<T, RegistryError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    pub id: String,
    pub version: String,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub permissions: Vec<String>,
    pub api_version: String,
    pub license: String,
    /// "js" | "declarative"
    pub kind: String,
    /// AI 가 생성한 코드인지 업로드자가 표기
    pub ai_generated: bool,
    /// 리뷰 통과(검증됨) 여부. 업로드 직후는 항상 false
    pub verified: bool,
    pub path: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Index {
    entries: Vec<Entry>,
}

#[derive(Debug, Clone, Default)]
pub struct PublishOptions {
    pub ai_generated: bool,
}

#[derive(Debug)]
pub struct PublishReport {
    pub entry: Entry,
    /// 비슷한 기존 플러그인(중복 가능성). 업로드는 막지 않고 알린다.
    pub similar: Vec<(String, f64)>,
    pub examples_checked: usize,
}

/// 검색 점수 계산기. 기본은 키워드, 나중에 임베딩 기반으로 교체한다.
pub trait Ranker {
    fn score(&self, query: &str, e: &Entry) -> f64;
}

pub struct KeywordRanker;

impl Ranker for KeywordRanker {
    fn score(&self, query: &str, e: &Entry) -> f64 {
        let q = query.to_lowercase();
        let terms: Vec<&str> = q.split_whitespace().collect();
        if terms.is_empty() {
            return 0.0;
        }
        let field = |s: &str| s.to_lowercase();
        let (id, name, desc) = (field(&e.id), field(&e.name), field(&e.description));
        let tags: Vec<String> = e.tags.iter().map(|t| field(t)).collect();
        let mut s = 0.0;
        for t in terms {
            if id == t || name == t {
                s += 10.0;
            }
            if name.contains(t) {
                s += 5.0;
            }
            if id.contains(t) {
                s += 4.0;
            }
            if tags.iter().any(|x| x == t) {
                s += 4.0;
            } else if tags.iter().any(|x| x.contains(t)) {
                s += 2.0;
            }
            if desc.contains(t) {
                s += 1.5;
            }
        }
        if e.verified {
            s *= 1.2;
        }
        s
    }
}

pub struct Registry {
    root: PathBuf,
}

impl Registry {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        std::fs::create_dir_all(root.join("plugins"))?;
        Ok(Registry { root })
    }

    fn index(&self) -> Result<Index> {
        let p = self.root.join("index.json");
        if !p.exists() {
            return Ok(Index::default());
        }
        Ok(serde_json::from_str(&std::fs::read_to_string(p)?)?)
    }

    fn save(&self, idx: &Index) -> Result<()> {
        // 쓰는 도중 중단돼도 index.json 이 깨지지 않도록 임시 파일에 쓰고 교체한다
        let tmp = self.root.join("index.json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(idx)?)?;
        std::fs::rename(tmp, self.root.join("index.json"))?;
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<Entry>> {
        Ok(self.index()?.entries)
    }

    /// 자동 검사 후 업로드.
    pub fn publish(&self, source_path: &Path, opts: &PublishOptions) -> Result<PublishReport> {
        let source = std::fs::read_to_string(source_path)?;
        let declarative = source.trim_start().starts_with('{');
        let (manifest, examples_checked, preview) = check_plugin(&source)?;

        let mut idx = self.index()?;
        if idx.entries.iter().any(|e| e.id == manifest.id && e.version == manifest.version) {
            return Err(RegistryError::DuplicateVersion(manifest.id, manifest.version));
        }
        let rel = format!("plugins/{}/{}", manifest.id, manifest.version);
        let dir = self.root.join(&rel);
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join(if declarative { "plugin.json" } else { "plugin.js" }), &source)?;
        std::fs::write(dir.join("manifest.json"), serde_json::to_string_pretty(&manifest)?)?;
        std::fs::write(dir.join("preview.svg"), preview)?;

        let entry = Entry {
            id: manifest.id.clone(),
            version: manifest.version.clone(),
            name: manifest.name.clone().unwrap_or_else(|| manifest.id.clone()),
            description: manifest.description.clone().unwrap_or_default(),
            tags: manifest.tags.clone(),
            permissions: manifest.permissions.clone(),
            api_version: manifest.api_version.clone(),
            license: manifest.license.clone().unwrap_or_else(|| "unspecified".into()),
            kind: if declarative { "declarative" } else { "js" }.into(),
            ai_generated: opts.ai_generated,
            verified: false,
            path: rel,
        };

        // 중복 가능성: 다른 id 인데 설명/태그가 크게 겹치는 것
        let mut similar: Vec<(String, f64)> = idx
            .entries
            .iter()
            .filter(|e| e.id != entry.id)
            .map(|e| (e.id.clone(), tag_overlap(&entry.tags, &e.tags) + if e.name == entry.name { 1.0 } else { 0.0 }))
            .filter(|(_, s)| *s >= 0.8)
            .collect();
        similar.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        idx.entries.push(entry.clone());
        self.save(&idx)?;
        Ok(PublishReport { entry, similar, examples_checked })
    }

    pub fn search(&self, query: &str, ranker: &dyn Ranker) -> Result<Vec<(Entry, f64)>> {
        let mut v: Vec<(Entry, f64)> = self
            .index()?
            .entries
            .into_iter()
            .map(|e| {
                let s = ranker.score(query, &e);
                (e, s)
            })
            .filter(|(_, s)| *s > 0.0)
            .collect();
        v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then_with(|| a.0.id.cmp(&b.0.id)));
        Ok(v)
    }

    /// id (그리고 선택적으로 version)로 가져온다. version 생략 시 최신(가장 마지막 업로드).
    /// 반환값은 import 전에 사용자에게 보여줄 권한/출처 정보를 포함한다.
    pub fn fetch(&self, id: &str, version: Option<&str>) -> Result<Fetched> {
        let idx = self.index()?;
        let e = idx
            .entries
            .iter()
            .rev()
            .find(|e| e.id == id && version.map_or(true, |v| e.version == v))
            .ok_or_else(|| RegistryError::NotFound(format!("{id}{}", version.map(|v| format!("@{v}")).unwrap_or_default())))?
            .clone();
        let dir = self.root.join(&e.path);
        let file = if e.kind == "js" { "plugin.js" } else { "plugin.json" };
        let source = std::fs::read_to_string(dir.join(file))?;
        let preview_svg = std::fs::read_to_string(dir.join("preview.svg")).unwrap_or_default();
        // 받은 직후에도 다시 검사한다(저장소가 변조됐을 가능성)
        check_plugin(&source)?;
        Ok(Fetched { entry: e, source, preview_svg })
    }

    /// 검증됨 뱃지는 리뷰어만 부여한다(앱 코드가 아니라 레포 운영 절차).
    pub fn mark_verified(&self, id: &str, version: &str) -> Result<()> {
        let mut idx = self.index()?;
        let e = idx.entries.iter_mut().find(|e| e.id == id && e.version == version).ok_or_else(|| RegistryError::NotFound(format!("{id}@{version}")))?;
        e.verified = true;
        self.save(&idx)
    }
}

#[derive(Debug)]
pub struct Fetched {
    pub entry: Entry,
    pub source: String,
    pub preview_svg: String,
}

fn tag_overlap(a: &[String], b: &[String]) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let n = a.iter().filter(|t| b.contains(t)).count() as f64;
    // 태그 1개만 겹치는 것(예: '재료역학')은 같은 분야일 뿐 중복이 아니다
    if n < 2.0 {
        return 0.0;
    }
    n / a.len().min(b.len()) as f64
}

/// 업로드/다운로드 공통 자동 검사. 격리된 호스트(권한 없음)에서 로드하고 모든 예제를 렌더한다.
/// 반환: (manifest, 검사한 렌더 횟수, 첫 렌더의 미리보기 SVG)
pub fn check_plugin(source: &str) -> Result<(Manifest, usize, String)> {
    let fail = |e: String| RegistryError::CheckFailed(e);
    let mut host = JsHost::new(); // 권한 미허용 → 선언된 권한이 있으면 로드 단계에서 거부
    let m = host.load(source).map_err(|e| fail(e.to_string()))?;
    if m.name.is_none() || m.description.is_none() || m.tags.is_empty() {
        return Err(fail("name, description, tags 는 업로드에 필수".into()));
    }
    if m.license.is_none() {
        return Err(fail("license 는 업로드에 필수 (MIT 또는 CC0 권장)".into()));
    }
    if !m.refs.is_empty() && m.examples.iter().all(|e| e.refs.is_empty()) {
        return Err(fail("연결(refs)이 필요한 플러그인은 refs 가 있는 예제가 필요".into()));
    }

    let mut cases: Vec<(Map<String, Value>, Map<String, Value>)> = vec![];
    if m.refs.is_empty() {
        cases.push((Map::new(), Map::new()));
    }
    for ex in &m.examples {
        let mut refs = ex.refs.clone();
        for r in refs.values_mut() {
            if let Some(o) = r.as_object_mut() {
                o.entry("offset").or_insert(serde_json::json!([0.0, 0.0]));
            }
        }
        cases.push((ex.params.clone(), refs));
    }
    let mut preview = String::new();
    for (i, (params, refs)) in cases.iter().enumerate() {
        let out = host.render_plugin(&m.id, params, refs).map_err(|e| fail(format!("예제 {i}: {e}")))?;
        if out.draws.is_empty() && out.attachments.is_empty() {
            return Err(fail(format!("예제 {i}: 아무것도 그리지 않음")));
        }
        if i == 0 {
            // 미리보기는 단일 객체 문서로 렌더. 연결이 있는 플러그인은 가짜 대상 객체를 함께 넣는다.
            let mut doc = Document::default();
            let mut obj = Object::new("preview", &m.id);
            obj.params = params.clone();
            doc.layers[0].objects.push(obj);
            if refs.is_empty() {
                if let Ok(r) = doc.render(&mut host) {
                    preview = to_svg(&r);
                }
            } else {
                preview = to_svg(&[note_core::RenderedObject { id: "preview".into(), output: out }]);
            }
        }
    }
    Ok((m, cases.len(), preview))
}
