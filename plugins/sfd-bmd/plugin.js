// 전단력도(SFD)/굽힘모멘트도(BMD). 'beam' 객체를 참조하며, 보의 값이 바뀌면 함께 재계산된다.
// 규약: V = R_A − ΣP(좌측). M 은 인장측(아래) 방향으로 양수를 아래로 그린다.
// 선도는 보와 같은 x 축 눈금을 쓰고, 보 객체의 위치 차이(offset)만큼 정렬된다.
export default definePlugin({
  id: 'sfd-bmd',
  version: '0.1.0',
  apiVersion: '0.1',
  name: 'SFD/BMD',
  description: '단순지지보의 전단력도와 굽힘모멘트도 (보 객체 참조)',
  tags: ['재료역학', '정역학', 'SFD', 'BMD'],
  license: 'MIT',
  permissions: [],
  params: {
    height: { default: 0.2, label: '선도 높이(L 대비)', min: 0.01 },
  },
  refs: { beam: { type: 'beam' } },
  examples: [
    { name: '중앙 집중하중', params: {}, refs: { beam: { type: 'beam', params: { L: 10, loads: [{ kind: 'point', x: 5, P: 20 }] } } } },
    { name: '등분포 + 집중', params: {}, refs: { beam: { type: 'beam', params: { L: 8, loads: [{ kind: 'udl', a: 2, b: 8, w: 3 }, { kind: 'point', x: 1, P: 10 }] } } } },
  ],
  render(p, ctx) {
    const beam = ctx.refs.beam, L = beam.params.L, loads = beam.params.loads;
    const [ox, oy] = beam.offset;

    // 반력: B 점 모멘트 평형
    let mA = 0, total = 0; // mA: A 에 대한 하중 모멘트
    for (const ld of loads) {
      if (ld.kind === 'point') { total += ld.P; mA += ld.P * ld.x; }
      else { const W = ld.w * (ld.b - ld.a); total += W; mA += W * (ld.a + ld.b) / 2; }
    }
    const RB = mA / L, RA = total - RB;

    const V = x => {
      let v = RA;
      for (const ld of loads) {
        if (ld.kind === 'point') { if (ld.x < x) v -= ld.P; }
        else v -= ld.w * Math.max(0, Math.min(x, ld.b) - ld.a);
      }
      return v;
    };
    const M = x => {
      let m = RA * x;
      for (const ld of loads) {
        if (ld.kind === 'point') { if (ld.x < x) m -= ld.P * (x - ld.x); }
        else { const e = Math.min(Math.max(x, ld.a), ld.b), W = ld.w * (e - ld.a); m -= W * (x - (ld.a + e) / 2); }
      }
      return m;
    };

    // 표본점: 균등 + 불연속점 양쪽
    const xs = new Set();
    for (let i = 0; i <= 200; i++) xs.add(L * i / 200);
    const eps = L * 1e-9, brk = [];
    for (const ld of loads) { if (ld.kind === 'point') brk.push(ld.x); else brk.push(ld.a, ld.b); }
    for (const b of brk) { xs.add(b); }
    const sorted = [...xs].sort((a, b) => a - b);
    const vPts = [];
    for (const x of sorted) {
      const isBrk = brk.some(b => Math.abs(b - x) < 1e-12);
      if (isBrk) { vPts.push([x, V(Math.max(0, x - eps))], [x, V(Math.min(L, x + eps))]); }
      else vPts.push([x, V(x)]);
    }
    let vmax = 0, mmax = 0, xm = 0;
    for (const [, v] of vPts) vmax = Math.max(vmax, Math.abs(v));
    for (const x of sorted) { if (Math.abs(M(x)) > Math.abs(mmax)) { mmax = M(x); xm = x; } }

    const H = L * p.height;
    const vs = vmax > 0 ? H / vmax : 0, ms = mmax !== 0 ? H / Math.abs(mmax) : 0;
    const yV = oy - L * 0.3, yM = yV - 3 * H;   // 보 아래로 SFD, 그 아래 BMD

    ctx.attach('axes', g => {
      g.line([ox, yV], [ox + L, yV], 'gray');
      g.line([ox, yM], [ox + L, yM], 'gray');
      for (const b of brk) { g.line([ox + b, oy], [ox + b, yM - H * 1.2], 'gray'); }
      g.line([ox, oy], [ox, yM - H * 1.2], 'gray');
      g.line([ox + L, oy], [ox + L, yM - H * 1.2], 'gray');
    });

    ctx.polyline([[ox, yV], ...vPts.map(([x, v]) => [ox + x, yV + v * vs]), [ox + L, yV]]);
    ctx.polyline([[ox, yM], ...sorted.map(x => [ox + x, yM - M(x) * ms]), [ox + L, yM]]);
    ctx.text([ox + L * 1.02, yV], 'SFD');
    ctx.text([ox + L * 1.02, yM], 'BMD');
    ctx.snap('Mmax', [ox + xm, yM - mmax * ms]);

    ctx.readout('R_A', RA, 'ΣM_B = 0');
    ctx.readout('R_B', RB, 'ΣM_A = 0');
    ctx.readout('|V|max', vmax, 'max |V(x)|');
    ctx.readout('M_max', mmax, 'max M(x)');
    ctx.readout('x @ M_max', xm, 'V(x)=0 지점');
  },
});
