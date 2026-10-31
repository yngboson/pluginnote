import SwiftUI
import PencilKit

/// 월드(y 위) ↔ 화면(y 아래) 변환. 1 월드 단위 = scale pt.
struct ViewXform {
    var scale: CGFloat = 8
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
                .gesture(SpatialTapGesture().onEnded { v in
                    if mode == .erase { eraseStroke(near: v.location, size: geo.size) }
                }, including: mode == .erase ? .all : .subviews)
                .gesture(MagnifyGesture().onChanged { v in
                    let s0 = zoomStart ?? xf.scale; zoomStart = s0
                    xf.scale = max(1, min(200, s0 * v.magnification))
                }.onEnded { _ in zoomStart = nil })

                InkLayer(enabled: mode == .ink) { pts in
                    // 화면 좌표 → 월드 좌표
                    let world = pts.map { p -> (x: Float, y: Float, p: Float) in
                        let w = xf.toWorld(CGPoint(x: CGFloat(p.x), y: CGFloat(p.y)), in: geo.size)
                        return (Float(w.x), Float(w.y), p.p)
                    }
                    store.addStroke(points: world, color: "#111111", width: 2)
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
                    if dragging != nil { store.checkpoint() }   // 드래그 한 번 = 되돌리기 한 번
                    if dragging == nil { panStart = xf.offset }
                }
                if let id = dragging { store.move(id, to: CGPoint(x: w.x, y: w.y)) }
                else if let p0 = panStart { xf.offset = CGSize(width: p0.width + v.translation.width, height: p0.height + v.translation.height) }
            }
            .onEnded { _ in dragging = nil; panStart = nil }
    }

    /// 터치 지점에서 가장 가까운 획(12pt 이내)을 코어에서 지운다
    private func eraseStroke(near s: CGPoint, size: CGSize) {
        var best: (Int, CGFloat)?
        for (i, st) in store.frame.strokes.enumerated() {
            for pt in st.points {
                let sp = xf.toScreen(pt, in: size)
                let d = hypot(sp.x - s.x, sp.y - s.y)
                if d < 12, d < (best?.1 ?? .infinity) { best = (i, d) }
            }
        }
        if let (i, _) = best { store.removeStroke(at: i) }
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
        guard s.count == 6, let v = UInt32(s, radix: 16) else { return .black }
        return Color(red: Double((v >> 16) & 255) / 255, green: Double((v >> 8) & 255) / 255, blue: Double(v & 255) / 255)
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
struct InkLayer: UIViewRepresentable {
    var enabled: Bool
    var onStroke: ([(x: Float, y: Float, p: Float)]) -> Void

    func makeUIView(context: Context) -> PKCanvasView {
        let v = PKCanvasView()
        v.backgroundColor = .clear
        v.isOpaque = false
        v.drawingPolicy = .pencilOnly            // 손가락은 팬/줌, 펜은 필기 (팜 리젝션 포함)
        v.tool = PKInkingTool(.pen, color: .black, width: 2)
        v.delegate = context.coordinator
        return v
    }
    func updateUIView(_ v: PKCanvasView, context: Context) {
        v.isUserInteractionEnabled = enabled
        context.coordinator.onStroke = onStroke
    }
    func makeCoordinator() -> Coordinator { Coordinator(onStroke) }

    final class Coordinator: NSObject, PKCanvasViewDelegate {
        var onStroke: ([(x: Float, y: Float, p: Float)]) -> Void
        private var clearing = false
        init(_ f: @escaping ([(x: Float, y: Float, p: Float)]) -> Void) { onStroke = f }
        func canvasViewDrawingDidChange(_ cv: PKCanvasView) {
            guard !clearing else { return }
            let strokes = cv.drawing.strokes
            guard !strokes.isEmpty else { return }
            // 끝난 획은 코어로 넘기고 PencilKit 화면은 비운다(이후엔 코어의 획을 Canvas 가 그린다)
            for s in strokes {
                let pts = s.path.map { (x: Float($0.location.x), y: Float($0.location.y), p: Float($0.force)) }
                if pts.count > 1 { onStroke(pts) }
            }
            clearing = true
            cv.drawing = PKDrawing()
            clearing = false
        }
    }
}
