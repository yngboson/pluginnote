import Foundation

// note-ffi `render()` 가 돌려주는 프레임 JSON 의 Swift 모델. 좌표는 월드(y 위쪽 양수).

struct Style: Codable {
    var stroke: String = "#111111"
    var width: Double = 1.5
    var dash: [Double]? = nil
    var fill: String? = nil
}

enum Draw: Decodable {
    case line(from: [Double], to: [Double], style: Style)
    case arrow(from: [Double], to: [Double], style: Style)
    case circle(c: [Double], r: Double, style: Style)
    case arc(c: [Double], r: Double, start: Double, end: Double, style: Style)
    case polyline(points: [[Double]], style: Style)
    case text(at: [Double], text: String, size: Double, style: Style)

    private enum K: String, CodingKey { case kind, from, to, c, r, start, end, points, at, text, size, style }

    init(from d: Decoder) throws {
        let k = try d.container(keyedBy: K.self)
        let style = try k.decodeIfPresent(Style.self, forKey: .style) ?? Style()
        switch try k.decode(String.self, forKey: .kind) {
        case "line": self = .line(from: try k.decode([Double].self, forKey: .from), to: try k.decode([Double].self, forKey: .to), style: style)
        case "arrow": self = .arrow(from: try k.decode([Double].self, forKey: .from), to: try k.decode([Double].self, forKey: .to), style: style)
        case "circle": self = .circle(c: try k.decode([Double].self, forKey: .c), r: try k.decode(Double.self, forKey: .r), style: style)
        case "arc": self = .arc(c: try k.decode([Double].self, forKey: .c), r: try k.decode(Double.self, forKey: .r), start: try k.decode(Double.self, forKey: .start), end: try k.decode(Double.self, forKey: .end), style: style)
        case "polyline": self = .polyline(points: try k.decode([[Double]].self, forKey: .points), style: style)
        case "text": self = .text(at: try k.decode([Double].self, forKey: .at), text: try k.decode(String.self, forKey: .text), size: try k.decodeIfPresent(Double.self, forKey: .size) ?? 12, style: style)
        default: throw DecodingError.dataCorruptedError(forKey: .kind, in: k, debugDescription: "unknown draw kind")
        }
    }
}

struct Attachment: Decodable { let name: String; let draws: [Draw] }
struct Snap: Decodable { let name: String; let at: [Double] }

struct JSONValue: Decodable, CustomStringConvertible {
    let raw: Any
    init(from d: Decoder) throws {
        let c = try d.singleValueContainer()
        if let v = try? c.decode(Double.self) { raw = v }
        else if let v = try? c.decode(String.self) { raw = v }
        else if let v = try? c.decode(Bool.self) { raw = v }
        else if let v = try? c.decode([JSONValue].self) { raw = v.map(\.raw) }
        else if let v = try? c.decode([String: JSONValue].self) { raw = v.mapValues(\.raw) }
        else { raw = NSNull() }
    }
    var isNumber: Bool { raw is Double }
    /// JSON 텍스트(편집 필드 표시/전송용)
    var jsonString: String {
        if raw is NSNull { return "null" }
        if let d = try? JSONSerialization.data(withJSONObject: raw, options: [.fragmentsAllowed, .sortedKeys]), let s = String(data: d, encoding: .utf8) { return s }
        return "null"
    }
    var description: String {
        if let v = raw as? Double { return String(format: "%.4g", v) }
        if let v = raw as? String { return v }
        return jsonString
    }
}

struct Readout: Decodable { let label: String; let value: JSONValue; let formula: String? }

struct RenderedObject: Decodable, Identifiable {
    let id: String
    let ok: Bool
    let error: String?
    let draws: [Draw]?
    let attachments: [Attachment]?
    let snaps: [Snap]?
    let readouts: [Readout]?
}

/// 코어에 보관된 잉크 획(월드 좌표). points 는 [x, y, 필압].
struct InkStroke: Decodable { let points: [[Double]]; let style: Style }

struct Frame: Decodable {
    let objects: [RenderedObject]
    let stroke_count: Int
    var strokes: [InkStroke] = []
    private enum CodingKeys: String, CodingKey { case objects, stroke_count, strokes }
    init(objects: [RenderedObject], stroke_count: Int) { self.objects = objects; self.stroke_count = stroke_count }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        objects = try c.decode([RenderedObject].self, forKey: .objects)
        stroke_count = try c.decode(Int.self, forKey: .stroke_count)
        strokes = try c.decodeIfPresent([InkStroke].self, forKey: .strokes) ?? []
    }
}

// manifest JSON (load_plugin 결과)
struct ParamSpec: Decodable { let `default`: JSONValue?; let label: String?; let min: Double?; let max: Double? }
struct Manifest: Decodable, Identifiable {
    let id: String
    let version: String
    let name: String?
    let description: String?
    let tags: [String]
    let permissions: [String]
    let params: [String: ParamSpec]
    let refs: [String: RefSpec]
    struct RefSpec: Decodable { let type: String }
    var title: String { name ?? id }
}
