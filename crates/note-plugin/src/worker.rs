//! 플러그인 호스트를 전용 스레드에서 돌리는 핸들. 호출 스레드(필기/UI)는 막히지 않고
//! `render_async` 로 결과 채널만 받는다. QuickJS 런타임은 이 스레드를 벗어나지 않는다.

use crate::{JsHost, Limits, Manifest, PluginError};
use note_core::{CoreError, PluginOutput, PluginRuntime};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::JoinHandle;

enum Msg {
    Load(String, Sender<Result<Manifest, PluginError>>),
    Render(String, Map<String, Value>, Map<String, Value>, Sender<Result<PluginOutput, PluginError>>),
    Quit,
}

pub struct WorkerHost {
    tx: Sender<Msg>,
    th: Option<JoinHandle<()>>,
    manifests: HashMap<String, Manifest>,
}

impl WorkerHost {
    pub fn spawn(limits: Limits, granted: &[&str]) -> Self {
        let granted: Vec<String> = granted.iter().map(|s| s.to_string()).collect();
        let (tx, rx) = channel::<Msg>();
        let th = std::thread::Builder::new()
            .name("plugin-host".into())
            .spawn(move || {
                let mut host = JsHost::new().with_limits(limits);
                for g in &granted {
                    host = host.grant(g);
                }
                while let Ok(m) = rx.recv() {
                    match m {
                        Msg::Load(src, r) => {
                            let _ = r.send(host.load(&src));
                        }
                        Msg::Render(id, p, refs, r) => {
                            let _ = r.send(host.render_plugin(&id, &p, &refs));
                        }
                        Msg::Quit => break,
                    }
                }
            })
            .expect("플러그인 스레드 시작 실패");
        WorkerHost { tx, th: Some(th), manifests: HashMap::new() }
    }

    pub fn load(&mut self, source: &str) -> Result<Manifest, PluginError> {
        let (rtx, rrx) = channel();
        self.tx.send(Msg::Load(source.into(), rtx)).map_err(|_| PluginError::Load("워커 종료됨".into()))?;
        let m = rrx.recv().map_err(|_| PluginError::Load("워커 응답 없음".into()))??;
        self.manifests.insert(m.id.clone(), m.clone());
        Ok(m)
    }

    /// 호출 스레드를 막지 않는다. 결과는 반환된 채널로 온다.
    pub fn render_async(&self, id: &str, params: &Map<String, Value>, refs: &Map<String, Value>) -> Receiver<Result<PluginOutput, PluginError>> {
        let (rtx, rrx) = channel();
        if self.tx.send(Msg::Render(id.into(), params.clone(), refs.clone(), rtx.clone())).is_err() {
            let _ = rtx.send(Err(PluginError::Render("워커 종료됨".into())));
        }
        rrx
    }

    pub fn manifest(&self, id: &str) -> Option<&Manifest> {
        self.manifests.get(id)
    }
}

impl Drop for WorkerHost {
    fn drop(&mut self) {
        let _ = self.tx.send(Msg::Quit);
        if let Some(t) = self.th.take() {
            let _ = t.join();
        }
    }
}

impl PluginRuntime for WorkerHost {
    fn has(&self, ty: &str) -> bool {
        self.manifests.contains_key(ty)
    }
    fn render(&mut self, ty: &str, params: &Map<String, Value>, refs: &Map<String, Value>) -> note_core::Result<PluginOutput> {
        self.render_async(ty, params, refs)
            .recv()
            .map_err(|_| CoreError::Plugin("워커 응답 없음".into()))?
            .map_err(|e| CoreError::Plugin(e.to_string()))
    }
}
