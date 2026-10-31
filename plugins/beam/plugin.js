// 단순지지보: 길이 L, 하중 목록. 지점은 x=0(힌지), x=L(롤러).
// loads: [{ kind:'point', x, P }, { kind:'udl', a, b, w }]  (P, w: 아래 방향이 양수)
export default definePlugin({
  id: 'beam',
  version: '0.1.0',
  apiVersion: '0.1',
  name: '단순지지보',
  description: '길이와 하중(집중, 등분포)을 가진 단순지지보. SFD/BMD 플러그인이 참조한다',
  tags: ['재료역학', '정역학', '보'],
  license: 'MIT',
  permissions: [],
  params: {
    L: { default: 10, label: '길이 L', min: 0.001 },
    loads: { default: [{ kind: 'point', x: 5, P: 20 }], label: '하중' },
  },
  examples: [
    { name: '중앙 집중하중', params: { L: 10, loads: [{ kind: 'point', x: 5, P: 20 }] } },
    { name: '등분포하중', params: { L: 10, loads: [{ kind: 'udl', a: 0, b: 10, w: 2 }] } },
  ],
  render(p, ctx) {
    const L = p.L, h = L * 0.04;
    ctx.line([0, 0], [L, 0], { stroke: '#111111', width: 4 });
    // 지점 기호
    ctx.polyline([[0, 0], [-h, -h * 1.6], [h, -h * 1.6], [0, 0]]);
    ctx.circle({ cx: L, cy: -h * 0.8, r: h * 0.8 });
    ctx.line([L - h * 1.6, -h * 1.6], [L + h * 1.6, -h * 1.6]);
    for (const ld of p.loads) {
      if (ld.kind === 'point') {
        ctx.arrow([ld.x, L * 0.18], [ld.x, 0.02 * L], 'accent');
        ctx.text([ld.x, L * 0.2], `${ld.P}`);
      } else if (ld.kind === 'udl') {
        const top = L * 0.1;
        ctx.line([ld.a, top], [ld.b, top], 'accent');
        const n = Math.max(2, Math.round((ld.b - ld.a) / L * 10));
        for (let i = 0; i <= n; i++) {
          const x = ld.a + (ld.b - ld.a) * i / n;
          ctx.arrow([x, top], [x, 0.01 * L], 'accent');
        }
        ctx.text([(ld.a + ld.b) / 2, top + L * 0.02], `w=${ld.w}`);
      } else throw new Error('unknown load kind: ' + ld.kind);
    }
    ctx.snap('A', [0, 0]);
    ctx.snap('B', [L, 0]);
  },
});
