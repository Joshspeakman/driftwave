import Foundation

/// Test hooks, used by scripts/smoke-test.sh:
///   SHORTWAVE_LOG=1          log player events to stderr
///   -playURL <url>           play a stream at launch (as a temporary local station)
///   -page discover|search    open on a page; -searchText <text> fills the search
///   -openFirstResult         show details of the first search result
///   -openTrackDetail         show details of the first track being recorded
///   -muted                   start muted (not saved)
enum Diagnostics {
    static let logging = ProcessInfo.processInfo.environment["SHORTWAVE_LOG"] == "1"

    static func log(_ message: @autoclosure () -> String) {
        guard logging else { return }
        FileHandle.standardError.write(Data("[shortwave] \(message())\n".utf8))
    }

    /// Test runs use their own data folder and preferences, so they never touch the real app's data.
    static var isTestRun: Bool {
        CommandLine.arguments.contains("-muted") || playURL != nil || startPage != nil || searchText != nil || openFirstResult || openTrackDetail
            || ProcessInfo.processInfo.environment["SHORTWAVE_TEST"] == "1"
    }

    static var playURL: String? { UserDefaults.standard.string(forKey: "playURL") }
    static var startPage: String? { UserDefaults.standard.string(forKey: "page") }
    static var searchText: String? { UserDefaults.standard.string(forKey: "searchText") }
    static var openFirstResult: Bool { CommandLine.arguments.contains("-openFirstResult") }
    static var openTrackDetail: Bool { CommandLine.arguments.contains("-openTrackDetail") }
    /// Test runs are always muted and keep off Now Playing / media keys (pass -audible for a sound test).
    static var startMuted: Bool {
        CommandLine.arguments.contains("-muted") || (isTestRun && !CommandLine.arguments.contains("-audible"))
    }
}
