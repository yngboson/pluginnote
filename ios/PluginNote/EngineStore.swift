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

    init() { loadBundledPlugins() }

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

    func move(_ id: String, to p: CGPoint) {
        do { try engine.moveObject(id: id, x: p.x, y: p.y); refresh() } catch { lastError = "\(error)" }
    }

    func remove(_ id: String) { _ = engine.removeObject(id: id); pendingIds.remove(id); if selection == id { selection = nil }; refresh() }

    func addStroke(points: [(x: Float, y: Float, p: Float)], color: String, width: Double) {
        let flat = points.flatMap { [$0.x, $0.y, $0.p] }
        try? engine.addStroke(flatXyp: flat, strokeColor: color, width: width)
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
