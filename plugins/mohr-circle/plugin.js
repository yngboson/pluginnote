// 모어원 (프리셋 v0.1): 원 + 중심의 수직/수평 보조선.
// 의도적으로 최소 기능만 둔다. 주응력/최대전단 표시, 숫자·키보드 입력 UI, 드래그 핸들은
// 같은 params(sx, sy, txy)와 `editor` 힌트 위에 이후 버전/별도 플러그인으로 얹는다.
export default definePlugin({
  id: 'mohr-circle',
  version: '0.1.0',
  apiVersion: '0.1',
  name: '모어원',
  description: '평면응력 상태(σx, σy, τxy)의 모어원과 중심 보조선',
  tags: ['재료역학', '고체역학', '응력'],
  license: 'MIT',
  permissions: [],
  params: {
    sx:  { default: 80,  label: 'σx',  editor: 'number' },
    sy:  { default: -40, label: 'σy',  editor: 'number' },
    txy: { default: 30,  label: 'τxy', editor: 'number' },
  },
  examples: [
    { name: '기본', params: { sx: 80, sy: -40, txy: 30 } },
    { name: '단축 인장', params: { sx: 100, sy: 0, txy: 0 } },
    { name: '순수 전단', params: { sx: 0, sy: 0, txy: 50 } },
  ],
  render(p, ctx) {
    const c = (p.sx + p.sy) / 2;
    const r = Math.hypot((p.sx - p.sy) / 2, p.txy);

    ctx.circle({ cx: c, cy: 0, r });

    // 중심 보조선: 부속 요소라서 원이 움직이거나 값이 바뀌면 함께 따라간다.
    const ext = r * 1.25 || 1;
    ctx.attach('center-guide-v', g => g.line([c, -ext], [c, ext], 'gray'));
    ctx.attach('center-guide-h', g => g.line([c - ext, 0], [c + ext, 0], 'gray'));

    ctx.snap('center', [c, 0]);
    ctx.readout('중심 C', c, '(σx + σy) / 2');
    ctx.readout('반지름 R', r, '√(((σx − σy)/2)² + τxy²)');
  },
});
