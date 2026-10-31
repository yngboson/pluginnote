// 플러그인 런타임 프렐류드. 플러그인마다 독립된 QuickJS 컨텍스트에서 가장 먼저 실행된다.
// 파일/네트워크/타이머 API는 존재하지 않는다(QuickJS 기본 글로벌만 있음).
(function (g) {
  const STYLES = {
    main:   { stroke: '#111111', width: 1.8 },
    gray:   { stroke: '#9a9a9a', width: 1.0, dash: [6, 4] },
    accent: { stroke: '#d33a2c', width: 1.8 },
  };
  function style(s) {
    if (s == null) return STYLES.main;
    if (typeof s === 'string') {
      if (!STYLES[s]) throw new Error('unknown style: ' + s);
      return STYLES[s];
    }
    return s;
  }
  function pt(p, what) {
    if (!Array.isArray(p) || p.length !== 2 || !isFinite(p[0]) || !isFinite(p[1]))
      throw new Error('invalid point for ' + what + ': ' + JSON.stringify(p));
    return [+p[0], +p[1]];
  }
  function num(v, what) {
    if (typeof v !== 'number' || !isFinite(v)) throw new Error('invalid number for ' + what + ': ' + v);
    return v;
  }
  function makeCtx(sink) {
    const ctx = {
      line(a, b, s)   { sink.draws.push({ kind: 'line', from: pt(a, 'line'), to: pt(b, 'line'), style: style(s) }); },
      arrow(a, b, s)  { sink.draws.push({ kind: 'arrow', from: pt(a, 'arrow'), to: pt(b, 'arrow'), style: style(s) }); },
      circle(o, s)    { sink.draws.push({ kind: 'circle', c: pt([o.cx, o.cy], 'circle'), r: num(o.r, 'circle.r'), style: style(s) }); },
      arc(o, s)       { sink.draws.push({ kind: 'arc', c: pt([o.cx, o.cy], 'arc'), r: num(o.r, 'arc.r'), start: num(o.start, 'arc.start'), end: num(o.end, 'arc.end'), style: style(s) }); },
      polyline(ps, s) { sink.draws.push({ kind: 'polyline', points: ps.map(p => pt(p, 'polyline')), style: style(s) }); },
      text(at, t, o)  { sink.draws.push({ kind: 'text', at: pt(at, 'text'), text: String(t), size: (o && o.size) || 12, style: style(o && o.style) }); },
    };
    if (sink.top) {
      // 부속 요소: 이름 붙은 하위 컨텍스트. 객체가 움직이거나 값이 바뀌면 함께 갱신된다.
      ctx.attach = function (name, fn) {
        const sub = { draws: [], top: false };
        fn(makeCtx(sub));
        sink.attachments.push({ name: String(name), draws: sub.draws });
      };
      ctx.snap = function (name, at) { sink.snaps.push({ name: String(name), at: pt(at, 'snap') }); };
      ctx.readout = function (label, value, formula) {
        sink.readouts.push({ label: String(label), value: value, formula: formula == null ? undefined : String(formula) });
      };
    }
    return ctx;
  }
  g.__plugin = null;
  g.definePlugin = function (spec) {
    if (!spec || typeof spec.render !== 'function') throw new Error('definePlugin: render 함수가 필요합니다');
    g.__plugin = spec;
    return spec;
  };
  g.__manifest = function () {
    const s = g.__plugin;
    if (!s) throw new Error('플러그인이 definePlugin 을 호출하지 않았습니다');
    return JSON.stringify({
      id: s.id, version: s.version, apiVersion: s.apiVersion,
      name: s.name, description: s.description, tags: s.tags, license: s.license,
      permissions: s.permissions || [],
      params: s.params || {},
      examples: s.examples || [],
    });
  };
  g.__render = function (paramsJson) {
    const sink = { draws: [], attachments: [], snaps: [], readouts: [], top: true };
    g.__plugin.render(JSON.parse(paramsJson), makeCtx(sink));
    return JSON.stringify({ draws: sink.draws, attachments: sink.attachments, snaps: sink.snaps, readouts: sink.readouts });
  };
})(globalThis);
