import Foundation
import SwiftUI

/// Rust 코어(NoteEngine)를 감싸는 상태 홀더. 규칙은 전부 Rust 에 있고 여기서는 호출과 갱신만 한다.
/// render() 는 플러그인 워커 스레드를 기다리므로 메인 스레드가 아니라 전용 큐에서 호출한다.
@MainActor
final class EngineStore: ObservableObject {
    @Published private(set) var frame = Frame(objects: [], stroke_count: 0)
    @Published private(set) var manifests: [Manifest] = []
    @Published var selection: String?
    @Published var lastError: String?

    private let engine = NoteEngine(grantedPermissions: [])   // 기본 무권한
    private let queue = DispatchQueue(label: "pluginnote.render", qos: .userInteractive)
    private var renderToken = 0

    init() { loadBundledPlugins(); loadPresets() }

    /// 시작 시 보여줄 예시 3개: 모어원, 보, 보에 연결된 SFD/BMD. load 로 넣으므로 되돌리기에 남지 않는다.
    private func loadPresets() {
        let doc = """
        {"format":"pluginnote","format_version":1,"layers":[{"id":"L1","name":"Layer 1","objects":[
         {"id":"mohr-circle-1","type":"mohr-circle","params":{"sx":40,"sy":-20,"txy":15},"origin":[-30,34]},
         {"id":"beam-1","type":"beam","params":{"L":30,"loads":[{"kind":"point","x":15,"P":20}]},"origin":[-30,-22]},
         {"id":"sfd-bmd-1","type":"sfd-bmd","origin":[-30,-22],"refs":{"beam":"beam-1"}}]}]}
        """
        load(json: doc)
    }

    // MARK: 플러그인
    func loadBundledPlugins() {
        guard let root = Bundle.main.url(forResource: "plugins", withExtension: nil),
              let dirs = try? FileManager.default.contentsOfDirectory(at: root, includingPropertiesForKeys: nil) else { return }
        for dir in dirs.sorted(by: { $0.lastPathComponent < $1.lastPathComponent }) {
            for name in ["plugin.js", "plugin.json"] {
                let f = dir.appendingPathComponent(name)
                if let src = try? String(contentsOf: f, encoding: .utf8) { install(source: src) }
            }
        }
    }

    /// 붙여넣기(외부 AI 코드) import 도 같은 경로. 설치 전에 inspect 로 권한을 보여준다.
    func inspect(source: String) -> Manifest? {
        do { return try decode(Manifest.self, engine.inspectPlugin(source: source)) }
        catch { lastError = "\(error)"; return nil }
    }

    func install(source: String) {
        do {
            let m = try decode(Manifest.self, engine.loadPlugin(source: source))
            manifests.removeAll { $0.id == m.id }
            manifests.append(m)
            manifests.sort { $0.title < $1.title }
        } catch { lastError = "플러그인 설치 실패: \(error)" }
    }

    func search(_ q: String) -> [Manifest] {
        let t = q.lowercased().split(separator: " ").map(String.init)
        guard !t.isEmpty else { return manifests }
        return manifests.filter { m in
            let hay = ([m.id, m.title, m.description ?? ""] + m.tags).joined(separator: " ").lowercased()
            return t.allSatisfy { hay.contains($0) }
        }
    }

    // MARK: 객체
    func add(type: String, at p: CGPoint) {
        var n = 1
        while frame.objects.contains(where: { $0.id == "\(type)-\(n)" }) || pendingIds.contains("\(type)-\(n)") { n += 1 }
        let id = "\(type)-\(n)"
        do {
            try engine.addObject(id: id, ty: type, paramsJson: "{}", x: p.x, y: p.y)
            pendingIds.insert(id)
            // 연결이 필요한 플러그인은 마지막으로 추가된 대상 type 객체에 자동 연결
            if let m = manifests.first(where: { $0.id == type }) {
                for (name, spec) in m.refs {
                    if let target = frame.objects.last(where: { $0.id.hasPrefix(spec.type + "-") }) {
                        try engine.setRef(id: id, name: name, target: target.id)
                    }
                }
            }
            selection = id
            refresh()
        } catch { lastError = "\(error)" }
    }
    private var pendingIds = Set<String>()

    func setParam(_ id: String, _ key: String, number: Double) {
        guard number.isFinite else { return }
        do { try engine.setParam(id: id, key: key, valueJson: "\(number)"); refresh() } catch { lastError = "\(error)" }
    }

    /// 배열/객체 등 임의 JSON 값 설정(예: 보의 하중 목록). 잘못된 JSON 은 코어 호출 전에 거부한다.
    @discardableResult
    func setParam(_ id: String, _ key: String, json: String) -> Bool {
        guard let d = json.data(using: .utf8), (try? JSONSerialization.jsonObject(with: d, options: [.fragmentsAllowed])) != nil else {
            lastError = "올바른 JSON 이 아닙니다"
            return false
        }
        do { try engine.setParam(id: id, key: key, valueJson: json); refresh(); return true } catch { lastError = "\(error)"; return false }
    }

    // MARK: 되돌리기 (객체/값/연결 변경은 코어 기록, 잉크 화면은 PencilKit 자체 undo)
    var canUndo: Bool { engine.canUndo() }
    var canRedo: Bool { engine.canRedo() }
    func checkpoint() { engine.checkpoint() }
    func undo() { if engine.undo() { selection = nil; pendingIds.removeAll(); refresh() } }
    func redo() { if engine.redo() { selection = nil; pendingIds.removeAll(); refresh() } }

    func move(_ id: String, to p: CGPoint) {
        do { try engine.moveObject(id: id, x: p.x, y: p.y); refresh() } catch { lastError = "\(error)" }
    }

    func remove(_ id: String) { _ = engine.removeObject(id: id); pendingIds.remove(id); if selection == id { selection = nil }; refresh() }

    /// points 는 월드 좌표. 코어가 보관하므로 확대/이동에 따라오고 되돌리기/지우개가 코어 기준으로 동작한다.
    func addStroke(points: [(x: Float, y: Float, p: Float)], color: String, width: Double) {
        let flat = points.flatMap { [$0.x, $0.y, $0.p] }
        do { try engine.addStroke(flatXyp: flat, strokeColor: color, width: width); refresh() } catch { lastError = "\(error)" }
    }

    func removeStroke(at index: Int) {
        guard engine.removeStroke(index: UInt32(index)) else { return }
        // 문지르며 지울 때 refresh 전에 다음 호출이 와도 인덱스가 어긋나지 않게 화면 쪽 목록도 바로 줄인다
        if frame.strokes.indices.contains(index) { frame.strokes.remove(at: index) }
        refresh()
    }

    // MARK: 문서에서 현재 값 읽기 (드래그 기준점, 속성 패널의 현재 값)
    private func docObject(_ id: String) -> [String: Any]? {
        guard let s = documentJSON(), let root = try? JSONSerialization.jsonObject(with: Data(s.utf8)) as? [String: Any],
              let layers = root["layers"] as? [[String: Any]] else { return nil }
        for l in layers { for o in l["objects"] as? [[String: Any]] ?? [] where o["id"] as? String == id { return o } }
        return nil
    }
    func origin(of id: String) -> CGPoint? {
        guard let a = docObject(id)?["origin"] as? [Double], a.count == 2 else { return nil }
        return CGPoint(x: a[0], y: a[1])
    }
    /// 저장된 값의 문자열. 값을 따로 정한 적이 없으면 nil (기본값을 쓰는 중)
    func paramText(_ id: String, _ key: String) -> String? {
        guard let v = (docObject(id)?["params"] as? [String: Any])?[key] else { return nil }
        if let n = v as? NSNumber { return "\(n)" }
        guard let d = try? JSONSerialization.data(withJSONObject: v, options: [.fragmentsAllowed]) else { return nil }
        return String(data: d, encoding: .utf8)
    }

    // MARK: 저장/내보내기
    func documentJSON() -> String? { try? engine.documentJson() }
    func exportSVG() -> String? { try? engine.exportSvg() }

    func load(json: String) {
        do { try engine.loadDocument(json: json); pendingIds.removeAll(); refresh() } catch { lastError = "\(error)" }
    }

    // MARK: 렌더 (오래된 결과는 버린다)
    func refresh() {
        renderToken += 1
        let token = renderToken
        queue.async { [engine] in
            let json = engine.render()
            let f = try? JSONDecoder().decode(Frame.self, from: Data(json.utf8))
            DispatchQueue.main.async { [weak self] in
                guard let self, token == self.renderToken, let f else { return }
                self.frame = f
                self.pendingIds.subtract(f.objects.map(\.id))
            }
        }
    }

    private func decode<T: Decodable>(_ t: T.Type, _ s: String) throws -> T { try JSONDecoder().decode(t, from: Data(s.utf8)) }
}
