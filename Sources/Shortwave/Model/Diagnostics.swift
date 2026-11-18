import Foundation

/// Test hooks, used by scripts/smoke-test.sh:
///   SHORTWAVE_LOG=1          log player events to stderr
///   -playURL <url>           play a stream at launch (as a temporary local station)
///   -page discover|search    open on a page; -searchText <text> fills the search
///   -openFirstResult         show details of the first search result
///   -openTrackDetail         show details of the first track being recorded
///   -muted                   start muted (not saved)
///   -importStations <file>   import a station file at launch; -exportStations <file> exports the library
///   -quitAfter <seconds>     quit after that long
///   -screenshot <png>        draw the main window (and sheet) to a PNG after -screenshotDelay s, then quit
///   -appearance light|dark   force the look
///   -miniPlayer, -addStation open the mini player / the Add Local Station sheet
enum Diagnostics {
    static let logging = ProcessInfo.processInfo.environment["SHORTWAVE_LOG"] == "1"

    static func log(_ message: @autoclosure () -> String) {
        guard logging else { return }
        FileHandle.standardError.write(Data("[shortwave] \(message())\n".utf8))
    }

    /// Test runs use their own data folder and preferences, so they never touch the real app's data.
    static var isTestRun: Bool {
        CommandLine.arguments.contains("-muted") || playURL != nil || startPage != nil || searchText != nil || openFirstResult || openTrackDetail
            || importStations != nil || exportStations != nil || quitAfter != nil || screenshot != nil || openMiniPlayer || openAddStation
            || ProcessInfo.processInfo.environment["SHORTWAVE_TEST"] == "1"
    }

    static var playURL: String? { UserDefaults.standard.string(forKey: "playURL") }
    static var startPage: String? { UserDefaults.standard.string(forKey: "page") }
    static var searchText: String? { UserDefaults.standard.string(forKey: "searchText") }
    static var importStations: String? { UserDefaults.standard.string(forKey: "importStations") }
    static var exportStations: String? { UserDefaults.standard.string(forKey: "exportStations") }
    static var quitAfter: Double? {
        let v = UserDefaults.standard.double(forKey: "quitAfter")
        return v > 0 ? v : nil
    }
    static var screenshot: String? { UserDefaults.standard.string(forKey: "screenshot") }
    static var screenshotDelay: Double? {
        let v = UserDefaults.standard.double(forKey: "screenshotDelay")
        return v > 0 ? v : nil
    }
    static var openMiniPlayer: Bool { CommandLine.arguments.contains("-miniPlayer") }
    static var openAddStation: Bool { CommandLine.arguments.contains("-addStation") }
    static var openFirstResult: Bool { CommandLine.arguments.contains("-openFirstResult") }
    static var openTrackDetail: Bool { CommandLine.arguments.contains("-openTrackDetail") }
    /// Test runs are always muted and keep off Now Playing / media keys (pass -audible for a sound test).
    static var startMuted: Bool {
        CommandLine.arguments.contains("-muted") || (isTestRun && !CommandLine.arguments.contains("-audible"))
    }
}
