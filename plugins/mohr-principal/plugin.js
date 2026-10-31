// 모어원 주석: 'mohr-circle' 객체를 참조해 주응력, 최대 전단응력, 2θp 를 얹는다.
// 프리셋(mohr-circle)은 건드리지 않는다 — 확장이 코어/프리셋 변경 없이 되는지 확인하는 용도.
// 규약: 점 X = (σx, −τxy) (τ 시계방향 양수를 아래쪽으로), 점 Y = (σy, +τxy).
export default definePlugin({
  id: 'mohr-principal',
  version: '0.1.0',
  apiVersion: '0.1',
  name: '모어원 주응력 표시',
  description: '모어원 객체에 주응력 σ1·σ2, 최대 전단응력, 2θp 를 표시하는 주석',
  tags: ['재료역학', '고체역학', '응력', '주응력'],
  license: 'MIT',
  permissions: [],
  params: {},
  refs: { circle: { type: 'mohr-circle' } },
  examples: [
    { name: '기본', params: {}, refs: { circle: { type: 'mohr-circle', params: { sx: 80, sy: -40, txy: 30 } } } },
    { name: '순수 전단', params: {}, refs: { circle: { type: 'mohr-circle', params: { sx: 0, sy: 0, txy: 50 } } } },
  ],
  render(p, ctx) {
    const c = ctx.refs.circle, q = c.params;
    const [ox, oy] = c.offset;
    const C = (q.sx + q.sy) / 2, dx = (q.sx - q.sy) / 2, R = Math.hypot(dx, q.txy);
    const s1 = C + R, s2 = C - R;
    const twoTheta = Math.atan2(q.txy, dx);         // rad, 2θp
    const X = [ox + q.sx, oy - q.txy], Y = [ox + q.sy, oy + q.txy], Cc = [ox + C, oy];

    ctx.line(Cc, X, 'accent');
    ctx.line(Cc, Y, 'accent');
    ctx.circle({ cx: X[0], cy: X[1], r: R * 0.02 }, 'accent');
    ctx.circle({ cx: Y[0], cy: Y[1], r: R * 0.02 }, 'accent');
    ctx.text([X[0], X[1] - R * 0.08], 'X');
    ctx.text([Y[0], Y[1] + R * 0.05], 'Y');

    // 주응력 점과 τmax 점
    for (const [x, y, label] of [[s1, 0, 'σ1'], [s2, 0, 'σ2'], [C, R, 'τmax'], [C, -R, '−τmax']]) {
      ctx.circle({ cx: ox + x, cy: oy + y, r: R * 0.02 });
      ctx.text([ox + x + R * 0.04, oy + y + R * 0.04], label);
    }
    // 2θp: X 방향(−2θp)에서 σ1 축(0)까지의 호
    if (R > 0 && twoTheta !== 0) {
      ctx.arc({ cx: Cc[0], cy: Cc[1], r: R * 0.3, start: -twoTheta, end: 0 }, 'accent');
      ctx.text([Cc[0] + R * 0.33, Cc[1] - R * 0.12], '2θp');
    }

    ctx.snap('sigma1', [ox + s1, oy]);
    ctx.snap('sigma2', [ox + s2, oy]);
    ctx.snap('tau_max', [ox + C, oy + R]);

    ctx.readout('σ1', s1, 'C + R');
    ctx.readout('σ2', s2, 'C − R');
    ctx.readout('τmax', R, 'R');
    ctx.readout('θp (deg)', twoTheta * 90 / Math.PI, '½·atan2(τxy, (σx−σy)/2)');
  },
});
