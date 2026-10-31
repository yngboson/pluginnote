use note_registry::{KeywordRanker, PublishOptions, Registry, RegistryError};
use std::path::{Path, PathBuf};

fn tmp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("pluginnote-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn plugin(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins").join(rel)
}

fn write(dir: &Path, name: &str, src: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, src).unwrap();
    p
}

fn seeded(name: &str) -> Registry {
    seeded_at(name).0
}

fn seeded_at(name: &str) -> (Registry, PathBuf) {
    let root = tmp(name).join("repo");
    let r = Registry::open(&root).unwrap();
    for f in ["mohr-circle/plugin.js", "beam/plugin.js", "sfd-bmd/plugin.js", "stress-strain/plugin.json", "fbd-arrow/plugin.js"] {
        r.publish(&plugin(f), &PublishOptions::default()).unwrap_or_else(|e| panic!("{f}: {e}"));
    }
    (r, root)
}

#[test]
fn publish_runs_checks_and_writes_files() {
    let r = seeded("pub");
    assert_eq!(r.list().unwrap().len(), 5);
    let f = r.fetch("mohr-circle", None).unwrap();
    assert!(f.preview_svg.contains("<circle"));
    assert_eq!(f.entry.kind, "js");
    assert!(!f.entry.verified);
    assert_eq!(r.fetch("stress-strain", None).unwrap().entry.kind, "declarative");
}

#[test]
fn search_finds_by_name_tag_and_description() {
    let r = seeded("search");
    let top = |q: &str| r.search(q, &KeywordRanker).unwrap().first().map(|(e, _)| e.id.clone());
    assert_eq!(top("모어원").as_deref(), Some("mohr-circle"));
    assert_eq!(top("SFD").as_deref(), Some("sfd-bmd"));
    assert_eq!(top("응력-변형률").as_deref(), Some("stress-strain"));
    assert_eq!(top("자유물체도 힘").as_deref().or(top("FBD").as_deref()), Some("fbd-arrow"));
    assert!(r.search("존재하지않는도식", &KeywordRanker).unwrap().is_empty());
}

#[test]
fn duplicate_version_is_rejected_new_version_is_fetched_latest() {
    let r = seeded("dup");
    assert!(matches!(r.publish(&plugin("mohr-circle/plugin.js"), &PublishOptions::default()), Err(RegistryError::DuplicateVersion(..))));
    let src = std::fs::read_to_string(plugin("mohr-circle/plugin.js")).unwrap().replace("version: '0.1.0'", "version: '0.2.0'");
    let d = tmp("dup2");
    r.publish(&write(&d, "m.js", &src), &PublishOptions::default()).unwrap();
    assert_eq!(r.fetch("mohr-circle", None).unwrap().entry.version, "0.2.0");
    assert_eq!(r.fetch("mohr-circle", Some("0.1.0")).unwrap().entry.version, "0.1.0");
}

#[test]
fn checks_reject_bad_uploads() {
    let r = Registry::open(tmp("bad").join("repo")).unwrap();
    let d = tmp("bad-src");
    let head = "id:'x', version:'1', apiVersion:'0.1', name:'x', description:'d', tags:['t'], license:'MIT'";
    let cases = [
        // 권한 선언
        format!("export default definePlugin({{ {head}, permissions:['network'], render(p,ctx){{ ctx.line([0,0],[1,1]); }} }});"),
        // 렌더 오류
        format!("export default definePlugin({{ {head}, render(p,ctx){{ throw new Error('boom'); }} }});"),
        // 아무것도 안 그림
        format!("export default definePlugin({{ {head}, render(p,ctx){{ }} }});"),
        // 무한 루프
        format!("export default definePlugin({{ {head}, render(p,ctx){{ while(true){{}} }} }});"),
        // 메타데이터 누락
        "export default definePlugin({ id:'x', version:'1', apiVersion:'0.1', render(p,ctx){ ctx.line([0,0],[1,1]); } });".into(),
        // 문법 오류
        "export default definePlugin({ ".into(),
        // 라이선스 누락
        "export default definePlugin({ id:'x', version:'1', apiVersion:'0.1', name:'x', description:'d', tags:['t'], render(p,ctx){ ctx.line([0,0],[1,1]); } });".into(),
    ];
    for (i, src) in cases.iter().enumerate() {
        let res = r.publish(&write(&d, &format!("c{i}.js"), src), &PublishOptions::default());
        assert!(matches!(res, Err(RegistryError::CheckFailed(_))), "case {i}: {res:?}");
    }
    assert!(r.list().unwrap().is_empty(), "실패한 업로드는 레포에 남지 않아야 함");
}

#[test]
fn ai_flag_and_verified_badge() {
    let r = Registry::open(tmp("ai").join("repo")).unwrap();
    r.publish(&plugin("fbd-arrow/plugin.js"), &PublishOptions { ai_generated: true }).unwrap();
    let e = r.fetch("fbd-arrow", None).unwrap().entry;
    assert!(e.ai_generated && !e.verified);
    r.mark_verified("fbd-arrow", "0.1.0").unwrap();
    assert!(r.fetch("fbd-arrow", None).unwrap().entry.verified);
}

#[test]
fn tampered_repo_is_caught_on_fetch() {
    let (r, root) = seeded_at("tamper");
    let f = r.fetch("fbd-arrow", None).unwrap();
    let evil = f.source.replace("permissions: []", "permissions: ['network']");
    assert_ne!(evil, f.source);
    // 레포 파일을 직접 덮어쓴다
    let file = root.join(&f.entry.path).join("plugin.js");
    assert!(file.exists());
    std::fs::write(file, evil).unwrap();
    assert!(matches!(r.fetch("fbd-arrow", None), Err(RegistryError::CheckFailed(_))));
}

#[test]
fn similar_plugin_is_reported_not_blocked() {
    let r = seeded("sim");
    let src = std::fs::read_to_string(plugin("mohr-circle/plugin.js")).unwrap().replace("id: 'mohr-circle'", "id: 'mohr-circle-2'");
    let rep = r.publish(&write(&tmp("sim-src"), "m2.js", &src), &PublishOptions::default()).unwrap();
    assert!(rep.similar.iter().any(|(id, _)| id == "mohr-circle"), "{:?}", rep.similar);
}
