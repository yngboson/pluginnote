# 인수인계: Mac 에서 이어서 하기

작성: 2026-10-31. 클라우드 세션(Linux, Xcode 없음)에서 작업하다 Mac 으로 넘기는 문서.
**새 Claude Code 세션이 이 레포를 열면 이 문서를 먼저 읽고, 아래 "첫 작업"부터 자동으로 진행한다.**

## 한 줄 상태

Rust 백엔드(코어/플러그인 호스트/레지스트리/FFI/CLI)는 CI(ubuntu)에서 통과했다.
**iPad 앱(`ios/`)은 소스만 작성되었고 한 번도 컴파일에 성공한 적이 없다.** 시뮬레이터 실행도 못 해봤다. 이것이 MVP 의 남은 핵심이다.

- 브랜치: `claude/magical-archimedes-h5usyx` (원격 push 됨)
- 사용자 선호: 한국어로 대화. 결정이 필요한 것만 물어보고 나머지는 진행한다.
- 앱 이름은 가칭 `pluginnote`.

## 첫 작업 (순서대로)

사전 준비: Xcode, `brew install xcodegen`, `rustup` 이 있어야 한다. 없으면 사용자에게 설치를 안내한다.

1. 브랜치 확인: `git checkout claude/magical-archimedes-h5usyx && git pull`
2. 백엔드 회귀 확인: `cargo test --workspace` (38개 이상 통과해야 정상)
3. iOS 빌드: `ios/build-ios.sh` (macOS 전용). **실패하면 에러를 읽고 고친다.** 아래 "이미 겪은 문제"를 먼저 본다.
4. `cd ios && xcodegen` 으로 프로젝트 생성 → `xcodebuild build -project PluginNote.xcodeproj -scheme PluginNote -destination 'generic/platform=iOS Simulator' CODE_SIGNING_ALLOWED=NO`
   (2026-11-01 Mac 로컬에서 3~5단계 완료: 빌드 스크립트의 `Generated/` 폴더 누락만 고쳤고 Swift 는 무수정 통과. 6단계 체크리스트가 남음)
5. **Swift 컴파일 에러를 모두 고친다.** `ios/PluginNote/*.swift` 는 Linux 에서 컴파일해 본 적이 없다. 생성 바인딩(`ios/PluginNote/Generated/note_ffi.swift`)의 실제 API 이름과 어긋난 곳이 있을 수 있다.
6. iPad 시뮬레이터에서 실행해 아래 "수동 확인 체크리스트"를 통과시킨다.
7. 고친 것을 커밋·푸시하고, 이 문서와 `docs/PLAN.md` 의 "미검증" 표기를 실제 상태로 갱신한다. (CI `ios` job 도 통과하는지 확인)

## 이미 겪은 문제 (같은 함정에 다시 빠지지 말 것)

CI(macos-14)의 `ios` job 으로 두 번 실패하고 고쳤다. 세 번째 시도(run #4~#6)는 결과를 보지 못하고 인수인계했다.

1. `rquickjs-sys` 는 iOS 용 사전 생성 바인딩이 없다 → `crates/note-plugin/Cargo.toml` 에서 **iOS 타깃에만** `bindgen` 기능을 켰다. 이 단계는 통과했다.
2. 링크 오류 `___chkstk_darwin` 미정의: QuickJS(C)는 iOS 17.5 로, Rust 링크는 10.0 으로 잡혀서 발생 → `ios/build-ios.sh` 에서 `IPHONEOS_DEPLOYMENT_TARGET=17.0` 으로 통일하고 `cargo rustc --crate-type staticlib` 로 정적 라이브러리만 빌드하도록 고쳤다. **이 수정은 CI 로 검증되지 못했다.** Mac 에서 먼저 확인할 것.
3. CI 의 `ios` job 이 한동안 `in_progress` 로 보이다 늦게 실패로 바뀌는 일이 반복됐다(상태 갱신 지연). 결과를 판단하기 전에 단계 로그가 열리는지 본다. 이제 `timeout-minutes` 와 단계별 시각 로그(`[build-ios]`)가 들어 있다.
4. `rquickjs` 는 `Context::base` 로는 프렐류드를 eval 할 수 없다 → `Context::full` 사용 중(I/O 글로벌은 없음, 테스트로 고정).

## 구조

```
crates/note-core      객체 모델, 저장 포맷(JSON, 값만 저장), 그리기 명령, SVG
crates/note-plugin    QuickJS 호스트(+선언형 JSON 플러그인), 샌드박스, 권한, WorkerHost(전용 스레드)
crates/note-registry  공유 레포(로컬 디렉터리 구현): 업로드 검사/검색/import
crates/note-ffi       UniFFI. Swift/Kotlin 이 호출하는 NoteEngine (전 데이터 JSON 문자열)
crates/note-cli       헤드리스 실행기 `pluginnote` (check/render/doc/registry)
plugins/              번들 플러그인 6종 (mohr-circle, mohr-principal, beam, sfd-bmd, stress-strain, fbd-arrow)
ios/                  iPad 앱 (SwiftUI + PencilKit), project.yml(XcodeGen), build-ios.sh
docs/                 PLAN.md(결정/계획), PLUGIN_API.md(API v0.1), 이 문서
```

원칙: 동작 규칙은 전부 Rust 에 있고 UI 는 얇게 유지한다. 잉크는 PencilKit 이 쓰는 동안만 표시하고, 획이 끝나면 월드 좌표로 코어에 저장한다.

## 결정 사항 (사용자 확정, 바꾸지 말 것)

- 1차 플랫폼: **iPad 먼저**, 안드로이드는 이후
- AI 플러그인 생성: **붙여넣기(import)만**. 앱 내장 AI 생성 없음
- 모어원 프리셋은 **원 + 중심 수직·수평 보조선만**. 주응력 등은 별도 플러그인(`mohr-principal`)으로 확장. 숫자/키보드·드래그 입력은 나중에 플러그인 쪽으로 확장
- 라이선스: 코어 MIT, 플러그인 기본 MIT 또는 CC0
- 선언형 JSON 플러그인 우선(앱스토어 정책 대비)

미결(지금 막히지 않음): 앱 이름, 공유 레포 운영 주체/서버, 약관 문구, 시맨틱 검색 서비스, 플러그인 서명 포맷.

## 검증 상태

| 항목 | 상태 |
|------|------|
| Rust 테스트 (core/plugin/registry/ffi) | CI ubuntu 통과. 로컬 38개+ (ffi 6개, plugin 28개, registry 7개) |
| 번들 플러그인 `pluginnote check` | CI 통과 |
| Swift/Kotlin 바인딩 생성 | CI 통과 (`scripts/gen-bindings.sh`) |
| 해석해 대조 (SFD/BMD, 모어원) | 테스트로 고정됨 |
| `ios/build-ios.sh` (iOS Rust 빌드) | **미검증** (두 번 실패 후 수정, 결과 못 봄) |
| Swift 앱 컴파일 | **미검증** (한 번도 컴파일 안 됨) |
| 시뮬레이터/실기기 실행 | **미검증** |
| 필기 지연 측정 | 미실시. 기기별 목표(ms)는 실기기에서 정한다 |

## 수동 확인 체크리스트 (시뮬레이터)

- [ ] 앱이 뜨고 번들 플러그인 6종이 "플러그인" 시트에 보인다(검색 "모어원", "SFD" 동작)
- [ ] 모어원 추가 → 원과 중심 수직·수평 보조선이 보인다. 인스펙터에서 σx/σy/τxy 입력 후 return 하면 원이 바뀐다
- [ ] 객체를 드래그하면 보조선이 함께 움직인다
- [ ] 보(beam) 추가 후 SFD/BMD 추가 → 자동 연결되어 선도가 보 아래에 정렬된다. 보의 하중(JSON 텍스트)을 바꾸면 선도가 바뀐다
- [ ] 필기 모드에서 획을 그리면 확대/이동 후에도 획이 따라온다. 지우개 모드에서 획을 탭하면 지워진다
- [ ] 되돌리기/다시하기가 객체·값·획에 동작한다
- [ ] 붙여넣기 import: 정상 플러그인은 설치되고, `permissions:['network']` 플러그인은 거부된다
- [ ] 저장(공유)과 열기(파일 앱)로 문서가 왕복된다
- [ ] 플러그인 하나가 오류여도 다른 객체는 그려진다 (프레임 단위 오류 분리)

## 그다음 할 일 (MVP 이후, `docs/PLAN.md` 와 동일)

- 필압을 굵기에 반영, 저장 포맷에 레이어/페이지 확장
- 숫자·키보드 입력 UI 를 `ParamSpec.editor` 힌트(slider 등)로 확장, 드래그 핸들 훅 설계
- API 동결(v1.0) 전 점검: 드래그 핸들 훅, 속성 편집 UI 스키마
- 공유 레포 원격 운영(서버, 서명, 신고·긴급 차단), 시맨틱 검색(`Ranker` 교체)
- 안드로이드(Kotlin) UI — 바인딩은 이미 생성된다
- 3순위 분야 플러그인(열유체/제어/전기/기구학)

## 작업 규칙

- 코어/플러그인 변경은 반드시 테스트와 함께. 해석해가 있는 도식은 해석해로 대조한다
- 확인하지 않은 것을 "됐다"고 쓰지 않는다. 상태 표기는 위 표처럼 검증 근거와 함께 쓴다
- 사용자가 요청하지 않으면 PR 을 만들지 않는다
- 커밋 메시지는 한국어, 끝에 세션 attribution 은 현재 세션의 지침을 따른다
