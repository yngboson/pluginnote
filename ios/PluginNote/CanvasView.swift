import SwiftUI
import PencilKit

/// 월드(y 위) ↔ 화면(y 아래) 변환. 1 월드 단위 = scale pt.
struct ViewXform {
    var scale: CGFloat = 6
    var offset: CGSize = .zero
    func toScreen(_ p: [Double], in size: CGSize) -> CGPoint {
        CGPoint(x: size.width / 2 + offset.width + CGFloat(p[0]) * scale,
                y: size.height / 2 + offset.height - CGFloat(p[1]) * scale)
    }
    func toWorld(_ s: CGPoint, in size: CGSize) -> CGPoint {
        CGPoint(x: (s.x - size.width / 2 - offset.width) / scale,
                y: -(s.y - size.height / 2 - offset.height) / scale)
    }
}

/// 객체 레이어(SwiftUI Canvas) + 잉크 레이어(PencilKit, 플랫폼 저지연 필기).
/// 잉크: PencilKit 은 쓰는 동안의 저지연 표시만 맡고, 획이 끝나면 월드 좌표로 변환해 코어에 저장한 뒤
/// 이 뷰의 Canvas 가 현재 변환(xf)으로 그린다 → 확대/이동에 따라오고, 지우개/되돌리기는 코어 기준이다.
struct CanvasView: View {
    @EnvironmentObject var store: EngineStore
    @State private var xf = ViewXform()
    @State private var panStart: CGSize?
    @State private var zoomStart: CGFloat?
    @State private var dragging: String?
    @State private var checkpointed = false     // 실제로 움직였을 때만 되돌리기 단계를 만든다(탭은 기록 안 함)
    @State private var grab = CGPoint.zero      // 잡은 지점과 객체 원점의 차이(탭만 해도 객체가 튀지 않게)
    @Binding var mode: Mode

    enum Mode: String, CaseIterable, Identifiable { case ink = "필기", object = "객체", erase = "지우개"; var id: String { rawValue } }

    var body: some View {
        GeometryReader { geo in
            ZStack {
                Canvas { ctx, size in
                    for st in store.frame.strokes where st.points.count > 1 {
                        var p = Path()
                        p.move(to: xf.toScreen(st.points[0], in: size))
                        st.points.dropFirst().forEach { p.addLine(to: xf.toScreen($0, in: size)) }
                        let (sh, ss) = stroke(st.style)
                        ctx.stroke(p, with: sh, style: ss)
                    }
                    for o in store.frame.objects where o.ok {
                        let sel = o.id == store.selection
                        for a in o.attachments ?? [] { for d in a.draws { draw(d, &ctx, size, dim: false) } }
                        for d in o.draws ?? [] { draw(d, &ctx, size, dim: false) }
                        if sel, let s = o.snaps {
                            for sp in s {
                                let c = xf.toScreen(sp.at, in: size)
                                ctx.stroke(Path(ellipseIn: CGRect(x: c.x - 4, y: c.y - 4, width: 8, height: 8)), with: .color(.blue), lineWidth: 1)
                            }
                        }
                    }
                }
                .background(Color(white: 0.98))
                .gesture(objectGestures(size: geo.size), including: mode == .object ? .all : .subviews)
                .gesture(DragGesture(minimumDistance: 0).onChanged { v in
                    eraseStroke(near: v.location, size: geo.size)   // 탭도, 문지르기도 지운다
                }, including: mode == .erase ? .all : .subviews)
                .gesture(MagnifyGesture().onChanged { v in
                    let s0 = zoomStart ?? xf.scale; zoomStart = s0
                    xf.scale = max(1, min(200, s0 * v.magnification))
                }.onEnded { _ in zoomStart = nil })

                // 도구 선택기(색/펜/지우개)는 필기·지우개 모드에서 보인다. 지우개를 고르면 지우개 모드로 전환.
                InkLayer(mode: mode, onEraserPicked: { isEraser in
                    if mode != .object { mode = isEraser ? .erase : .ink }
                }) { pts, color, width in
                    // 화면 좌표 → 월드 좌표
                    let world = pts.map { p -> (x: Float, y: Float, p: Float) in
                        let w = xf.toWorld(CGPoint(x: CGFloat(p.x), y: CGFloat(p.y)), in: geo.size)
                        return (Float(w.x), Float(w.y), p.p)
                    }
                    store.addStroke(points: world, color: color, width: width)
                }
                .allowsHitTesting(mode == .ink)
            }
        }
    }

    private func objectGestures(size: CGSize) -> some Gesture {
        DragGesture(minimumDistance: 0)
            .onChanged { v in
                let w = xf.toWorld(v.location, in: size)
                if dragging == nil {
                    // 선택: 가장 가까운 스냅/그림 점 근처
                    dragging = pick(at: v.startLocation, size: size)
                    store.selection = dragging
                    if let id = dragging {
                        let s = xf.toWorld(v.startLocation, in: size)
                        let o = store.origin(of: id) ?? s
                        grab = CGPoint(x: o.x - s.x, y: o.y - s.y)
                    } else { panStart = xf.offset }
                }
                if let id = dragging {
                    if !checkpointed, hypot(v.translation.width, v.translation.height) > 3 { store.checkpoint(); checkpointed = true }   // 드래그 한 번 = 되돌리기 한 번
                    if checkpointed { store.move(id, to: CGPoint(x: w.x + grab.x, y: w.y + grab.y)) }
                }
                else if let p0 = panStart { xf.offset = CGSize(width: p0.width + v.translation.width, height: p0.height + v.translation.height) }
            }
            .onEnded { _ in dragging = nil; panStart = nil; checkpointed = false }
    }

    /// 터치 지점에서 가장 가까운 획(16pt 이내)을 코어에서 지운다. 점이 아니라 선분 거리로 잰다
    /// (PencilKit 은 직선을 점 몇 개로만 저장하므로 점 거리로는 선 위를 눌러도 안 지워진다).
    private func eraseStroke(near s: CGPoint, size: CGSize) {
        var best: (Int, CGFloat)?
        for (i, st) in store.frame.strokes.enumerated() {
            let sp = st.points.map { xf.toScreen($0, in: size) }
            for k in 0..<max(sp.count - 1, 0) {
                let d = distance(from: s, toSegment: sp[k], sp[k + 1])
                if d < 16, d < (best?.1 ?? .infinity) { best = (i, d) }
            }
        }
        if let (i, _) = best { store.removeStroke(at: i) }
    }

    private func distance(from p: CGPoint, toSegment a: CGPoint, _ b: CGPoint) -> CGFloat {
        let dx = b.x - a.x, dy = b.y - a.y
        let len2 = dx * dx + dy * dy
        let t = len2 == 0 ? 0 : max(0, min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / len2))
        return hypot(p.x - (a.x + t * dx), p.y - (a.y + t * dy))
    }

    /// 그려진 점들 중 터치와 가까운 객체의 id
    private func pick(at s: CGPoint, size: CGSize) -> String? {
        var best: (String, CGFloat)?
        for o in store.frame.objects where o.ok {
            for d in o.draws ?? [] {
                for p in points(of: d) {
                    let sp = xf.toScreen(p, in: size)
                    let dist = hypot(sp.x - s.x, sp.y - s.y)
                    if dist < 24, dist < (best?.1 ?? .infinity) { best = (o.id, dist) }
                }
            }
        }
        return best?.0
    }

    private func points(of d: Draw) -> [[Double]] {
        switch d {
        case .line(let a, let b, _), .arrow(let a, let b, _): return [a, b, [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2]]
        case .circle(let c, let r, _), .arc(let c, let r, _, _, _): return [[c[0] + r, c[1]], [c[0] - r, c[1]], [c[0], c[1] + r], [c[0], c[1] - r]]
        case .polyline(let p, _): return p
        case .text(let at, _, _, _): return [at]
        }
    }

    private func color(_ hex: String) -> Color {
        var s = hex; if s.hasPrefix("#") { s.removeFirst() }
        guard s.count == 6 || s.count == 8, let v = UInt32(s, radix: 16) else { return .black }
        let a = s.count == 8 ? v & 255 : 255        // #RRGGBBAA (형광펜 등 반투명)
        let rgb = s.count == 8 ? v >> 8 : v
        return Color(.sRGB, red: Double((rgb >> 16) & 255) / 255, green: Double((rgb >> 8) & 255) / 255, blue: Double(rgb & 255) / 255, opacity: Double(a) / 255)
    }

    private func stroke(_ st: Style) -> (GraphicsContext.Shading, StrokeStyle) {
        (.color(color(st.stroke)), StrokeStyle(lineWidth: st.width, lineCap: .round, lineJoin: .round, dash: (st.dash ?? []).map { CGFloat($0) }))
    }

    private func draw(_ d: Draw, _ ctx: inout GraphicsContext, _ size: CGSize, dim: Bool) {
        func P(_ p: [Double]) -> CGPoint { xf.toScreen(p, in: size) }
        switch d {
        case .line(let a, let b, let st):
            var p = Path(); p.move(to: P(a)); p.addLine(to: P(b))
            let (sh, ss) = stroke(st); ctx.stroke(p, with: sh, style: ss)
        case .arrow(let a, let b, let st):
            let pa = P(a), pb = P(b)
            var p = Path(); p.move(to: pa); p.addLine(to: pb)
            let (sh, ss) = stroke(st); ctx.stroke(p, with: sh, style: ss)
            let len = hypot(pb.x - pa.x, pb.y - pa.y)
            if len > 1 {
                let ux = (pb.x - pa.x) / len, uy = (pb.y - pa.y) / len, h = min(10, len / 2)
                var head = Path()
                head.move(to: pb)
                head.addLine(to: CGPoint(x: pb.x - ux * h - uy * h * 0.4, y: pb.y - uy * h + ux * h * 0.4))
                head.addLine(to: CGPoint(x: pb.x - ux * h + uy * h * 0.4, y: pb.y - uy * h - ux * h * 0.4))
                head.closeSubpath()
                ctx.fill(head, with: sh)
            }
        case .circle(let c, let r, let st):
            let pc = P(c), rr = CGFloat(r) * xf.scale
            let (sh, ss) = stroke(st)
            ctx.stroke(Path(ellipseIn: CGRect(x: pc.x - rr, y: pc.y - rr, width: rr * 2, height: rr * 2)), with: sh, style: ss)
        case .arc(let c, let r, let s, let e, let st):
            let pc = P(c), rr = CGFloat(r) * xf.scale
            var p = Path()
            // 월드는 y 위, 화면은 y 아래 → 각도 부호 반전, 반시계(월드) = clockwise:false 가 아님에 주의
            p.addArc(center: pc, radius: rr, startAngle: .radians(-s), endAngle: .radians(-e), clockwise: true)
            let (sh, ss) = stroke(st); ctx.stroke(p, with: sh, style: ss)
        case .polyline(let pts, let st):
            guard let f = pts.first else { return }
            var p = Path(); p.move(to: P(f)); pts.dropFirst().forEach { p.addLine(to: P($0)) }
            let (sh, ss) = stroke(st); ctx.stroke(p, with: sh, style: ss)
        case .text(let at, let t, let size, let st):
            ctx.draw(Text(t).font(.system(size: CGFloat(size))).foregroundColor(color(st.stroke)), at: P(at), anchor: .bottomLeading)
        }
    }
}

/// PencilKit 잉크 레이어. 렌더링은 PencilKit 이 하고(저지연), 끝난 획만 코어로 보관한다.
/// 색/펜 종류/굵기/지우개는 Apple 의 PKToolPicker(메모 앱과 같은 팔레트)를 그대로 쓴다.
/// shortcut: 펜 종류별 질감(연필/크레용)은 저장하지 않고 색·굵기·투명도만 보관, 질감이 필요해지면 Style 에 ink 종류를 추가.
struct InkLayer: UIViewRepresentable {
    var mode: CanvasView.Mode
    var onEraserPicked: (Bool) -> Void
    var onStroke: ([(x: Float, y: Float, p: Float)], String, Double) -> Void

    /// 되돌리기는 상단 툴바(코어 기준)만 쓴다. 팔레트의 되돌리기 버튼은 비워 둔 PencilKit 화면을 대상으로 하므로 꺼 둔다.
    final class InkCanvas: PKCanvasView { override var undoManager: UndoManager? { nil } }

    func makeUIView(context: Context) -> PKCanvasView {
        let v = InkCanvas()
        v.backgroundColor = .clear
        v.isOpaque = false
        v.overrideUserInterfaceStyle = .light    // 캔버스 배경이 항상 밝으므로 잉크 색도 라이트 기준
        v.drawingPolicy = .default               // 시스템 설정(Pencil 만 쓰기 등)을 따른다. Pencil 이 없으면 손가락으로도 쓴다
        v.delegate = context.coordinator
        let picker = PKToolPicker()
        picker.overrideUserInterfaceStyle = .light
        picker.addObserver(v)
        picker.addObserver(context.coordinator)
        context.coordinator.picker = picker
        context.coordinator.canvas = v
        return v
    }
    func updateUIView(_ v: PKCanvasView, context: Context) {
        let c = context.coordinator
        c.onStroke = onStroke
        c.onEraserPicked = onEraserPicked
        let mode = mode
        // 뷰가 윈도우에 붙은 뒤에야 first responder 가 될 수 있으므로 다음 턴에 처리
        DispatchQueue.main.async {
            guard let picker = c.picker else { return }
            let show = mode != .object
            picker.setVisible(show, forFirstResponder: v)
            if show { v.becomeFirstResponder() } else { v.resignFirstResponder() }
            // 상단 모드와 팔레트의 도구를 일치시킨다. 팔레트는 마지막에 쓴 도구(지우개 등)를 기억한 채 시작하므로
            // 필기 모드인데 지우개가 선택돼 있으면 아무리 써도 획이 남지 않는다.
            let isEraser = v.tool is PKEraserTool
            if mode == .ink, isEraser || v.tool is PKLassoTool { Self.select(inking: true, picker, v) }
            else if mode == .erase, !isEraser { Self.select(inking: false, picker, v) }
        }
    }
    func makeCoordinator() -> Coordinator { Coordinator(onStroke, onEraserPicked) }

    private static func select(inking: Bool, _ picker: PKToolPicker, _ v: PKCanvasView) {
        if #available(iOS 18.0, *) {
            let item = picker.toolItems.first { inking ? $0 is PKToolPickerInkingItem : $0 is PKToolPickerEraserItem }
            if let item { picker.selectedToolItem = item; return }
        }
        v.tool = inking ? PKInkingTool(.pen, color: .black, width: 2) : PKEraserTool(.vector)
    }

    final class Coordinator: NSObject, PKCanvasViewDelegate, PKToolPickerObserver {
        var onStroke: ([(x: Float, y: Float, p: Float)], String, Double) -> Void
        var onEraserPicked: (Bool) -> Void
        var picker: PKToolPicker?
        weak var canvas: PKCanvasView?
        private var clearing = false
        private var usingTool = false
        init(_ f: @escaping ([(x: Float, y: Float, p: Float)], String, Double) -> Void, _ e: @escaping (Bool) -> Void) { onStroke = f; onEraserPicked = e }

        // 도구 선택기에서 도구를 바꿨을 때. 캔버스가 먼저 tool 을 갱신하도록 한 턴 뒤에 읽는다.
        func toolPickerSelectedToolDidChange(_ toolPicker: PKToolPicker) { notifyTool() }
        @available(iOS 18.0, *)
        func toolPickerSelectedToolItemDidChange(_ toolPicker: PKToolPicker) { notifyTool() }
        private func notifyTool() {
            DispatchQueue.main.async { [weak self] in
                guard let tool = self?.canvas?.tool else { return }
                self?.onEraserPicked(tool is PKEraserTool)
            }
        }

        // 획이 진행 중일 때 화면을 비우면 획이 끊긴다. 끝난 뒤에만 처리한다.
        // drawingDidChange 는 DidEndUsingTool 보다 늦게 올 수 있으므로 양쪽에서 시도한다(처리는 한 번만 일어난다).
        func canvasViewDidBeginUsingTool(_ cv: PKCanvasView) { usingTool = true }
        func canvasViewDidEndUsingTool(_ cv: PKCanvasView) {
            usingTool = false
            DispatchQueue.main.async { [weak self, weak cv] in if let cv { self?.flush(cv) } }
        }
        func canvasViewDrawingDidChange(_ cv: PKCanvasView) { if !usingTool { flush(cv) } }

        private func flush(_ cv: PKCanvasView) {
            guard !clearing else { return }
            let strokes = cv.drawing.strokes
            guard !strokes.isEmpty else { return }
            // 끝난 획은 코어로 넘기고 PencilKit 화면은 비운다(이후엔 코어의 획을 Canvas 가 그린다)
            for s in strokes {
                // 제어점만 이으면 꺾여 보이므로 곡선 위를 3pt 간격으로 다시 뽑는다
                let dense = Array(s.path.interpolatedPoints(by: .distance(3)))
                let pts = dense.map { (x: Float($0.location.x), y: Float($0.location.y), p: Float($0.force)) }
                guard pts.count > 1 else { continue }
                let widths = dense.map { Double($0.size.width) }
                let avg = widths.reduce(0, +) / Double(widths.count)
                onStroke(pts, Self.hex(s.ink.color, marker: s.ink.inkType == .marker), max(1, min(40, avg)))
            }
            clearing = true
            cv.drawing = PKDrawing()
            clearing = false
        }

        /// UIColor → #RRGGBB (형광펜은 반투명 #RRGGBBAA)
        static func hex(_ c: UIColor, marker: Bool) -> String {
            var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
            c.resolvedColor(with: UITraitCollection(userInterfaceStyle: .light)).getRed(&r, green: &g, blue: &b, alpha: &a)
            let h = String(format: "#%02X%02X%02X", Int(r * 255 + 0.5), Int(g * 255 + 0.5), Int(b * 255 + 0.5))
            return marker ? h + String(format: "%02X", Int((min(a, 0.35)) * 255 + 0.5)) : h
        }
    }
}
