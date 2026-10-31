# 플러그인 API v0.1 (초안)

API 버전은 `0.1`. 주(major)가 다르면 로드를 거부한다. 1.0 전에는 호환성을 보장하지 않는다(동결 게이트는 PLAN.md 참고).

## 형식

| 형식 | 파일 | 용도 |
|------|------|------|
| 선언형 | `plugin.json` | 코드 실행 없음. 수식 평가기만 사용. **가능하면 이 형식을 쓴다** |
| 코드형 | `plugin.js` | ES 모듈, `export default definePlugin({...})`. QuickJS 샌드박스에서 실행 |

로더는 소스가 `{`로 시작하면 선언형, 아니면 코드형으로 판단한다.

## 공통 필드 (manifest)

| 필드 | 필수(업로드) | 설명 |
|------|---|------|
| `id`, `version`, `apiVersion` | ✓ | `version` 은 시맨틱 버전 |
| `name`, `description`, `tags` | ✓ | 검색에 쓰인다 |
| `license` | ✓ | MIT 또는 CC0 권장 |
| `permissions` | | 기본 `[]`. 선언한 권한은 사용자가 허용해야 로드된다 |
| `params` | | `{ key: { default, label?, min?, max?, editor? } }`. 기본값 채움, 범위 검증, 모르는 키 거부 |
| `refs` | | `{ name: { type } }` 다른 객체에 대한 연결. type 이 다르면 거부 |
| `examples` | | 업로드 자동 검사에 쓰인다. `refs` 가 있는 플러그인은 `refs` 가 든 예제 필수 |

`params[*].editor`(`"number"`, `"slider"`)는 UI 힌트다. 숫자/키보드 입력, 슬라이더, 드래그 핸들 같은
입력 방식은 이 값과 `ctx.snap` 위에서 UI 가 구성하고, 플러그인은 값 → 그림만 책임진다.

## 코드형: `render(p, ctx)`

`p` 는 기본값이 채워진 파라미터. 좌표는 객체 로컬(y 위쪽 양수), 각도는 라디안(반시계 양수).
스타일은 `'main' | 'gray' | 'accent'` 또는 `{stroke, width, dash?, fill?}`.

| 호출 | 설명 |
|------|------|
| `ctx.line(a, b, style?)` `ctx.arrow(a, b, style?)` | 점은 `[x, y]` |
| `ctx.circle({cx, cy, r}, style?)` | |
| `ctx.arc({cx, cy, r, start, end}, style?)` | start → end 반시계 |
| `ctx.polyline(points, style?)` | |
| `ctx.text(at, str, {size?, style?}?)` | |
| `ctx.attach(name, g => { ... })` | **부속 요소**. `g` 는 위와 같은 그리기 메서드. 값이 바뀌거나 객체가 움직이면 자동으로 따라간다 |
| `ctx.snap(name, [x, y])` | 스냅 포인트 등록 |
| `ctx.readout(label, value, formula?)` | 계산 결과 표시. 계산식을 함께 노출해 사용자가 검증할 수 있게 한다 |
| `ctx.refs.<name>` | 연결된 객체 `{id, type, params, offset}`. `params` 는 대상 플러그인의 기본값이 채워져 있다. `offset` 은 (대상 원점 − 내 원점)이라 위치 정렬에 쓴다 |

입력 좌표가 유한하지 않거나(NaN, Infinity) 알 수 없는 스타일이면 렌더 오류가 된다.

## 선언형 스펙

`draw`/`attach[].draw` 의 연산: `line, arrow, circle, arc, polyline, text`. 좌표/길이는 **수식 문자열**.
`vars` 는 순서대로 평가되는 중간 변수, `snaps`, `readouts` 도 수식을 쓴다.

수식: 숫자, 변수(params, vars), `+ - * / ^`, 괄호, 상수 `pi`,
함수 `sin cos tan atan atan2 sqrt hypot abs exp ln min max pow`. 그 외 문자/함수/미정의 변수는 오류, 결과가 유한하지 않으면 오류.

## 객체 간 연결 (refs)

문서의 객체는 `refs: { 이름: "대상 id" }` 를 가진다. 렌더 시 호스트가 대상의 `type/params/offset` 을 넘긴다.
대상 값이 바뀌거나 대상을 옮기면 참조하는 객체가 다음 렌더에서 함께 갱신된다. 예: `sfd-bmd → beam`, `mohr-principal → mohr-circle`.

## 샌드박스와 한도

- 플러그인마다 독립된 QuickJS 런타임. 파일/네트워크/타이머 글로벌 없음
- 기본값: 호출당 200ms, 메모리 32MiB, 스택 512KiB. 초과 시 해당 호출만 실패
- 앱에서는 전용 워커 스레드에서 실행(`WorkerHost`) — UI/필기 스레드는 결과 채널만 받는다
- 업로드·다운로드 시 권한 없는 격리 호스트에서 예제를 렌더해 검사한다. 선언된 권한이 있으면 거부된다
