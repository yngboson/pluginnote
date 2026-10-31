//! note-plugin: QuickJS 기반 플러그인 호스트.
//!
//! - 플러그인마다 독립 Runtime/Context (전역 오염 차단)
//! - 메모리/스택/실행시간 상한, 초과 시 해당 플러그인 호출만 실패
//! - 기본 무권한: manifest 의 permissions 는 호스트가 허용한 것만 통과

mod declarative;
mod manifest;
mod worker;

pub use worker::WorkerHost;

pub use manifest::{Example, Manifest, ParamSpec, RefSpec, API_VERSION};

use note_core::{CoreError, PluginOutput, PluginRuntime};
use rquickjs::{CatchResultExt, Context, Function, Module, Runtime};
use serde_json::{Map, Value};
use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use thiserror::Error;

const PRELUDE: &str = include_str!("prelude.js");

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("플러그인 로드 실패: {0}")]
    Load(String),
    #[error("manifest 오류: {0}")]
    Manifest(String),
    #[error("권한 거부: 선언된 권한 {0:?} 는 허용되지 않음")]
    Permission(Vec<String>),
    #[error("API 버전 비호환: 플러그인 {plugin}, 호스트 {host}")]
    ApiVersion { plugin: String, host: String },
    #[error("파라미터 오류: {0}")]
    Params(String),
    #[error("렌더 오류: {0}")]
    Render(String),
    #[error("시간 초과 ({0:?})")]
    Timeout(Duration),
}

#[derive(Debug, Clone)]
pub struct Limits {
    pub memory_bytes: usize,
    pub stack_bytes: usize,
    pub time_per_call: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Limits { memory_bytes: 32 << 20, stack_bytes: 512 << 10, time_per_call: Duration::from_millis(200) }
    }
}

enum Backend {
    Js {
        // Runtime 은 Context 보다 오래 살아야 한다
        _rt: Runtime,
        ctx: Context,
        deadline: Arc<Mutex<Option<Instant>>>,
    },
    Declarative(Box<declarative::Spec>),
}

struct Loaded {
    manifest: Manifest,
    backend: Backend,
}

pub struct JsHost {
    plugins: HashMap<String, Loaded>,
    granted: BTreeSet<String>,
    limits: Limits,
    /// 시간 초과한 (플러그인, 입력). 같은 입력으로 매 프레임 200ms 를 또 쓰지 않도록 즉시 실패시킨다
    timed_out: BTreeSet<String>,
}

impl Default for JsHost {
    fn default() -> Self {
        Self::new()
    }
}

impl JsHost {
    pub fn new() -> Self {
        JsHost { plugins: HashMap::new(), granted: BTreeSet::new(), limits: Limits::default(), timed_out: BTreeSet::new() }
    }

    pub fn with_limits(mut self, l: Limits) -> Self {
        self.limits = l;
        self
    }

    /// 호스트가 허용하는 권한. 기본은 비어 있다.
    pub fn grant(mut self, perm: &str) -> Self {
        self.granted.insert(perm.into());
        self
    }

    pub fn manifest(&self, id: &str) -> Option<&Manifest> {
        self.plugins.get(id).map(|p| &p.manifest)
    }

    /// 소스가 `{` 로 시작하면 선언형 JSON 스펙, 아니면 JS 모듈로 로드한다.
    pub fn load(&mut self, source: &str) -> Result<Manifest, PluginError> {
        if source.trim_start().starts_with('{') {
            self.load_declarative(source)
        } else {
            self.load_js(source)
        }
    }

    fn check_manifest(&self, manifest: &Manifest) -> Result<(), PluginError> {
        if manifest.id.is_empty() || manifest.version.is_empty() {
            return Err(PluginError::Manifest("id 와 version 은 필수".into()));
        }
        // id/version 은 레지스트리 폴더 이름으로 쓰이므로 경로 문자를 막는다
        let safe = |s: &str| !s.starts_with('.') && s.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c));
        if !safe(&manifest.id) || !safe(&manifest.version) {
            return Err(PluginError::Manifest("id/version 은 영문·숫자·-_. 만 허용".into()));
        }
        // 설치된 플러그인을 다른 버전이 조용히 덮어쓰지 못하게 한다(같은 버전 재로드는 허용)
        if self.plugins.get(&manifest.id).is_some_and(|p| p.manifest.version != manifest.version) {
            return Err(PluginError::Manifest(format!("이미 설치된 id: {} (버전이 다름)", manifest.id)));
        }
        if !manifest.api_compatible() {
            return Err(PluginError::ApiVersion { plugin: manifest.api_version.clone(), host: API_VERSION.into() });
        }
        let denied: Vec<String> = manifest.permissions.iter().filter(|p| !self.granted.contains(*p)).cloned().collect();
        if !denied.is_empty() {
            return Err(PluginError::Permission(denied));
        }
        Ok(())
    }

    fn load_declarative(&mut self, source: &str) -> Result<Manifest, PluginError> {
        let spec: declarative::Spec = serde_json::from_str(source).map_err(|e| PluginError::Manifest(e.to_string()))?;
        self.check_manifest(&spec.manifest)?;
        let m = spec.manifest.clone();
        self.plugins.insert(m.id.clone(), Loaded { manifest: m.clone(), backend: Backend::Declarative(Box::new(spec)) });
        Ok(m)
    }

    /// JS 소스를 로드한다. `export default definePlugin({...})` 형태의 ES 모듈.
    fn load_js(&mut self, source: &str) -> Result<Manifest, PluginError> {
        let rt = Runtime::new().map_err(|e| PluginError::Load(e.to_string()))?;
        rt.set_memory_limit(self.limits.memory_bytes);
        rt.set_max_stack_size(self.limits.stack_bytes);
        let deadline: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
        let dl = deadline.clone();
        rt.set_interrupt_handler(Some(Box::new(move || dl.lock().unwrap().map_or(false, |d| Instant::now() > d))));
        let ctx = Context::full(&rt).map_err(|e| PluginError::Load(e.to_string()))?;

        let budget = self.limits.time_per_call;
        let manifest_json: String = ctx
            .with(|c| -> Result<String, String> {
                *deadline.lock().unwrap() = Some(Instant::now() + budget);
                c.eval::<(), _>(PRELUDE).catch(&c).map_err(|e| e.to_string())?;
                let p = Module::evaluate(c.clone(), "plugin", source).catch(&c).map_err(|e| e.to_string())?;
                p.finish::<()>().catch(&c).map_err(|e| e.to_string())?;
                let f: Function = c.globals().get("__manifest").map_err(|e| e.to_string())?;
                f.call::<_, String>(()).catch(&c).map_err(|e| e.to_string())
            })
            .map_err(PluginError::Load)?;

        let manifest: Manifest = serde_json::from_str(&manifest_json).map_err(|e| PluginError::Manifest(e.to_string()))?;
        self.check_manifest(&manifest)?;
        self.plugins.insert(manifest.id.clone(), Loaded { manifest: manifest.clone(), backend: Backend::Js { _rt: rt, ctx, deadline } });
        Ok(manifest)
    }

    pub fn render_plugin(&mut self, id: &str, params: &Map<String, Value>, refs: &Map<String, Value>) -> Result<PluginOutput, PluginError> {
        let budget = self.limits.time_per_call;
        let p = self.plugins.get(id).ok_or_else(|| PluginError::Render(format!("로드되지 않은 플러그인: {id}")))?;
        let resolved = p.manifest.resolve_params(params).map_err(PluginError::Params)?;
        // 연결 대상의 params 에도 대상 플러그인의 기본값을 채워서 넘긴다(대상이 값을 비워 둬도 참조 쪽이 안전하게 읽도록)
        let mut refs = refs.clone();
        for r in refs.values_mut() {
            let ty = r.get("type").and_then(Value::as_str).map(str::to_owned);
            let given = r.get("params").and_then(Value::as_object).cloned().unwrap_or_default();
            if let Some(target) = ty.and_then(|t| self.plugins.get(&t)) {
                let full = target.manifest.resolve_params(&given).map_err(|e| PluginError::Params(format!("연결 대상: {e}")))?;
                r["params"] = Value::Object(full);
            }
        }
        let refs = &refs;
        p.manifest.check_refs(refs).map_err(PluginError::Params)?;
        let (rt_ctx, deadline) = match &p.backend {
            Backend::Declarative(spec) => return declarative::render(spec, &resolved).map_err(PluginError::Render),
            Backend::Js { ctx, deadline, .. } => (ctx, deadline),
        };
        let json = Value::Object(resolved).to_string();
        let refs_json = Value::Object(refs.clone()).to_string();
        let key = format!("{id}\0{json}\0{refs_json}");
        if self.timed_out.contains(&key) {
            return Err(PluginError::Timeout(budget));
        }
        *deadline.lock().unwrap() = Some(Instant::now() + budget);
        let started = Instant::now();
        let res = rt_ctx.with(|c| -> Result<String, String> {
            let f: Function = c.globals().get("__render").map_err(|e| e.to_string())?;
            f.call::<_, String>((json, refs_json)).catch(&c).map_err(|e| e.to_string())
        });
        *deadline.lock().unwrap() = None;
        match res {
            Ok(s) => serde_json::from_str(&s).map_err(|e| PluginError::Render(e.to_string())),
            Err(_) if started.elapsed() >= budget => {
                if self.timed_out.len() >= 256 {
                    self.timed_out.clear();
                }
                self.timed_out.insert(key);
                Err(PluginError::Timeout(budget))
            }
            Err(e) => Err(PluginError::Render(e)),
        }
    }
}

impl PluginRuntime for JsHost {
    fn has(&self, ty: &str) -> bool {
        self.plugins.contains_key(ty)
    }
    fn render(&mut self, ty: &str, params: &Map<String, Value>, refs: &Map<String, Value>) -> note_core::Result<PluginOutput> {
        self.render_plugin(ty, params, refs).map_err(|e| CoreError::Plugin(e.to_string()))
    }
}
