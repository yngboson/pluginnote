# pluginnote (가칭)

필기와 기본 도형만 코어로 제공하고, 공학 작도(모어원, SFD/BMD 등)는 스크립트 플러그인으로 확장하는 공학용 노트 앱.
기획: [docs/PLAN.md](docs/PLAN.md) · 이어서 작업하기: [docs/HANDOFF.md](docs/HANDOFF.md)

```
crates/note-core    객체 모델, 저장 포맷, 그리기 명령, SVG
crates/note-plugin  QuickJS 플러그인 호스트 (샌드박스, 권한, 한도)
crates/note-registry 공유 레포(로컬): 업로드 검사, 검색, import
crates/note-ffi     UniFFI 바인딩 (Swift/Kotlin) — iPad 앱이 호출하는 `NoteEngine`
crates/note-cli     헤드리스 실행기 `pluginnote`
plugins/            번들 플러그인: 모어원, 모어원 주응력, 보, SFD/BMD, 응력-변형률, FBD 화살표
examples/           문서 예제 (SFD/BMD, 모어원 전체)
docs/               PLAN.md(결정·계획), PLUGIN_API.md
```

```bash
cargo test
```

```bash
cargo run -p note-cli -- check plugins/mohr-circle/plugin.js
```

```bash
cargo run -p note-cli -- render plugins/mohr-circle/plugin.js sx=80 sy=-40 txy=30 -o mohr.svg
```

```bash
cargo run -p note-cli -- doc plugins/beam/plugin.js plugins/sfd-bmd/plugin.js -d examples/sfd-bmd.json -o sfd.svg
```

```bash
cargo run -p note-cli -- registry ./repo publish plugins/mohr-circle/plugin.js
```

```bash
scripts/gen-bindings.sh
```

License: MIT
