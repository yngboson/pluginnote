# pluginnote (가칭)

필기와 기본 도형만 코어로 제공하고, 공학 작도(모어원, SFD/BMD 등)는 스크립트 플러그인으로 확장하는 공학용 노트 앱.
기획: [docs/PLAN.md](docs/PLAN.md)

```
crates/note-core    객체 모델, 저장 포맷, 그리기 명령, SVG
crates/note-plugin  QuickJS 플러그인 호스트 (샌드박스, 권한, 한도)
crates/note-cli     헤드리스 실행기 `pluginnote`
plugins/            예제 플러그인 (모어원, FBD 화살표)
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

License: MIT
