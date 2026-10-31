// 자유물체도 힘 화살표. 모어원과 다른 유형(주석형)으로 API가 한 플러그인에 과적합되지 않았는지 확인하는 용도.
export default definePlugin({
  id: 'fbd-arrow',
  version: '0.1.0',
  apiVersion: '0.1',
  name: 'FBD 힘 화살표',
  description: '크기에 비례한 힘 벡터와 x/y 분해 성분',
  tags: ['동역학', '정역학', 'FBD'],
  license: 'MIT',
  permissions: [],
  params: {
    magnitude: { default: 10, label: '크기 F', min: 0 },
    angle:     { default: 30, label: '각도 (deg)' },
    scale:     { default: 0.1, label: '길이 배율', min: 0 },
  },
  examples: [{ name: '기본', params: {} }],
  render(p, ctx) {
    const a = p.angle * Math.PI / 180;
    const L = p.magnitude * p.scale;
    const fx = L * Math.cos(a), fy = L * Math.sin(a);
    ctx.arrow([0, 0], [fx, fy], 'accent');
    ctx.text([fx, fy], `F = ${p.magnitude}`, { size: 12 });
    ctx.attach('components', g => {
      g.arrow([0, 0], [fx, 0], 'gray');
      g.arrow([0, 0], [0, fy], 'gray');
      g.line([fx, 0], [fx, fy], 'gray');
      g.line([0, fy], [fx, fy], 'gray');
    });
    ctx.snap('tail', [0, 0]);
    ctx.snap('tip', [fx, fy]);
    ctx.readout('Fx', p.magnitude * Math.cos(a), 'F·cosθ');
    ctx.readout('Fy', p.magnitude * Math.sin(a), 'F·sinθ');
  },
});
