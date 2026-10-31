import SwiftUI

@main
struct PluginNoteApp: App {
    @StateObject private var store = EngineStore()
    var body: some Scene {
        WindowGroup { ContentView().environmentObject(store) }
    }
}
