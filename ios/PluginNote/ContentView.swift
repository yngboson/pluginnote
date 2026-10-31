import SwiftUI

struct ContentView: View {
    @EnvironmentObject var store: EngineStore
    @State private var mode: CanvasView.Mode = .object
    @State private var showLibrary = false
    @State private var showPaste = false

    var body: some View {
        NavigationSplitView {
            Inspector()
                .navigationTitle("속성")
        } detail: {
            CanvasView(mode: $mode)
                .toolbar {
                    ToolbarItem(placement: .principal) {
                        Picker("모드", selection: $mode) {
                            ForEach(CanvasView.Mode.allCases) { Text($0.rawValue).tag($0) }
                        }.pickerStyle(.segmented).frame(width: 180)
                    }
                    ToolbarItemGroup(placement: .primaryAction) {
                        Button { showLibrary = true } label: { Label("플러그인", systemImage: "puzzlepiece.extension") }
                        Button { showPaste = true } label: { Label("붙여넣기", systemImage: "doc.on.clipboard") }
                        ShareLink(item: store.documentJSON() ?? "{}", preview: SharePreview("pluginnote.json")) { Label("저장", systemImage: "square.and.arrow.up") }
                    }
                }
        }
        .sheet(isPresented: $showLibrary) { LibraryView().environmentObject(store) }
        .sheet(isPresented: $showPaste) { PasteImportView().environmentObject(store) }
        .alert("오류", isPresented: Binding(get: { store.lastError != nil }, set: { if !$0 { store.lastError = nil } })) {
            Button("확인", role: .cancel) {}
        } message: { Text(store.lastError ?? "") }
    }
}

/// 선택된 객체의 숫자/키보드 입력 + 계산 결과(계산식 포함).
struct Inspector: View {
    @EnvironmentObject var store: EngineStore

    var body: some View {
        List {
            if let id = store.selection, let obj = store.frame.objects.first(where: { $0.id == id }) {
                let type = id.split(separator: "-").dropLast().joined(separator: "-")
                let m = store.manifests.first(where: { $0.id == type })
                Section("\(id)") {
                    if let m {
                        ForEach(m.params.keys.sorted(), id: \.self) { key in
                            ParamField(objectID: id, key: key, spec: m.params[key]!)
                        }
                        if m.params.isEmpty { Text("편집할 값 없음").foregroundStyle(.secondary) }
                    }
                    if !obj.ok { Text(obj.error ?? "오류").foregroundStyle(.red).font(.caption) }
                }
                if let r = obj.readouts, !r.isEmpty {
                    Section("결과") {
                        ForEach(r.indices, id: \.self) { i in
                            VStack(alignment: .leading) {
                                HStack { Text(r[i].label); Spacer(); Text(r[i].value.description).monospacedDigit() }
                                if let f = r[i].formula { Text(f).font(.caption2).foregroundStyle(.secondary) }   // 계산식 노출
                            }
                        }
                    }
                }
                Section { Button("삭제", role: .destructive) { store.remove(id) } }
            } else {
                Text("객체를 선택하거나, 상단의 플러그인에서 추가하세요").foregroundStyle(.secondary)
            }
        }
    }
}

struct ParamField: View {
    @EnvironmentObject var store: EngineStore
    let objectID: String
    let key: String
    let spec: ParamSpec
    @State private var text = ""

    var body: some View {
        // 배열 등 숫자가 아닌 기본값(예: 보의 하중 목록)은 v0 UI 에서 편집하지 않는다
        if spec.default?.raw is Double {
            HStack {
                Text(spec.label ?? key)
                TextField(spec.default?.description ?? "", text: $text)
                    .keyboardType(.numbersAndPunctuation)
                    .multilineTextAlignment(.trailing)
                    .onSubmit { if let v = Double(text) { store.setParam(objectID, key, number: v) } }
            }
        } else {
            Text("\(spec.label ?? key): 편집 미지원(v0)").font(.caption).foregroundStyle(.secondary)
        }
    }
}

struct LibraryView: View {
    @EnvironmentObject var store: EngineStore
    @Environment(\.dismiss) var dismiss
    @State private var query = ""

    var body: some View {
        NavigationStack {
            List(store.search(query)) { m in
                Button {
                    store.add(type: m.id, at: .zero)
                    dismiss()
                } label: {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(m.title).font(.headline)
                        Text(m.description ?? "").font(.subheadline).foregroundStyle(.secondary)
                        HStack {
                            ForEach(m.tags, id: \.self) { Text($0).font(.caption2).padding(.horizontal, 6).background(.quaternary, in: Capsule()) }
                            Spacer()
                            Text(m.permissions.isEmpty ? "권한 없음" : "권한: " + m.permissions.joined(separator: ", ")).font(.caption2)
                        }
                    }
                }
            }
            .searchable(text: $query, prompt: "모어원, SFD, 응력…")
            .navigationTitle("플러그인")
            .toolbar { Button("닫기") { dismiss() } }
            .overlay { if store.search(query).isEmpty { Text("결과 없음 — 외부 AI 로 만든 코드를 붙여넣기로 추가하세요").foregroundStyle(.secondary) } }
        }
    }
}

/// 외부 AI 가 만든 코드 붙여넣기 import. 설치 전에 격리 검사(inspect)로 권한을 먼저 보여준다.
struct PasteImportView: View {
    @EnvironmentObject var store: EngineStore
    @Environment(\.dismiss) var dismiss
    @State private var source = ""
    @State private var preview: Manifest?

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading) {
                TextEditor(text: $source).font(.system(.footnote, design: .monospaced)).border(.quaternary)
                if let m = preview {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("\(m.title) v\(m.version)").font(.headline)
                        Text(m.description ?? "")
                        Text(m.permissions.isEmpty ? "요구 권한: 없음" : "요구 권한: \(m.permissions.joined(separator: ", "))")
                    }.padding(.vertical, 8)
                }
                HStack {
                    Button("검사") { preview = store.inspect(source: source) }
                    Spacer()
                    Button("설치") { store.install(source: source); dismiss() }.disabled(preview == nil).buttonStyle(.borderedProminent)
                }
            }
            .padding()
            .navigationTitle("붙여넣기")
            .toolbar { Button("닫기") { dismiss() } }
        }
    }
}
