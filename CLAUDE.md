# pluginnote

공학용 확장형 노트 앱(가칭). Rust 코어 + QuickJS 플러그인 + iPad UI(SwiftUI/PencilKit).

**이 레포에서 새 세션을 시작하면 먼저 `docs/HANDOFF.md` 를 읽고, 거기의 "첫 작업"부터 진행한다.**
(Mac + Xcode 환경이면 `ios/build-ios.sh` → `xcodegen` → `xcodebuild` 로 iPad 앱 컴파일을 맞추는 것이 첫 목표다.)

- 결정 사항과 계획: `docs/PLAN.md`, 플러그인 API: `docs/PLUGIN_API.md`
- 테스트: `cargo test --workspace`, 번들 플러그인 검사: `cargo run -q -p note-cli -- check plugins/*/plugin.js plugins/*/plugin.json`
- 사용자와는 한국어로 대화한다. 결정이 필요한 일만 묻는다.
- 검증하지 않은 것을 완료라고 보고하지 않는다.
