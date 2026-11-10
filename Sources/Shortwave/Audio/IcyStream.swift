import Foundation
import ShortwaveCore

/// A second connection to a Shoutcast/Icecast stream that reads its in-band song titles and
/// records tracks. AVPlayer plays the audio but does not surface ICY titles on macOS.
final class IcyStream: NSObject, URLSessionDataDelegate, @unchecked Sendable {
    enum Message {
        /// Connected. `hasTitles` is false when the server sends no ICY metadata.
        case connected(hasTitles: Bool, format: RecordableFormat?)
        case recorder([SegmentRecorder.Event])
        /// The connection ended without being asked to.
        case ended(String?)
    }

    private let url: URL
    private let directory: URL
    private let onMessage: @MainActor (Message) -> Void
    private let queue: OperationQueue
    private var session: URLSession!
    private var task: URLSessionDataTask?

    // Touched only on `queue`.
    private var parser: ICYParser?
    private var recorder: SegmentRecorder?
    private var config: SegmentRecorder.Config
    private var stopped = false

    init(url: URL, config: SegmentRecorder.Config, directory: URL, onMessage: @escaping @MainActor (Message) -> Void) {
        self.url = url
        self.config = config
        self.directory = directory
        self.onMessage = onMessage
        queue = OperationQueue()
        queue.maxConcurrentOperationCount = 1
        queue.name = "IcyStream"
        super.init()
        let cfg = URLSessionConfiguration.ephemeral
        cfg.timeoutIntervalForRequest = 20
        cfg.timeoutIntervalForResource = .infinity
        cfg.requestCachePolicy = .reloadIgnoringLocalCacheData
        session = URLSession(configuration: cfg, delegate: self, delegateQueue: queue)
    }

    func start() {
        var req = URLRequest(url: url)
        req.setValue("1", forHTTPHeaderField: "Icy-MetaData")
        req.setValue(RadioBrowser.userAgent, forHTTPHeaderField: "User-Agent")
        task = session.dataTask(with: req)
        task?.resume()
    }

    func stop() {
        queue.addOperation { [self] in
            stopped = true
            if let events = recorder?.streamEnded(), !events.isEmpty { post(.recorder(events)) }
            task?.cancel()
            session.invalidateAndCancel()
        }
    }

    func update(config: SegmentRecorder.Config) {
        queue.addOperation { [self] in
            self.config = config
            recorder?.config = config
        }
    }

    func cancelRecording() {
        queue.addOperation { [self] in
            if let events = recorder?.cancel(), !events.isEmpty { post(.recorder(events)) }
        }
    }

    private func post(_ m: Message) {
        let handler = onMessage
        // DispatchQueue.main keeps messages in order (separate Tasks need not run in order).
        DispatchQueue.main.async { MainActor.assumeIsolated { handler(m) } }
    }

    // MARK: URLSessionDataDelegate (on `queue`)

    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive response: URLResponse,
                    completionHandler: @escaping (URLSession.ResponseDisposition) -> Void) {
        guard !stopped, let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) else {
            completionHandler(.cancel)
            return
        }
        let format = RecordableFormat(contentType: http.value(forHTTPHeaderField: "Content-Type"))
        guard let metaInt = http.value(forHTTPHeaderField: "icy-metaint").flatMap({ Int($0.trimmingCharacters(in: .whitespaces)) }),
              metaInt > 0 else {
            stopped = true
            post(.connected(hasTitles: false, format: format))
            completionHandler(.cancel)
            return
        }
        parser = ICYParser(metaInt: metaInt)
        recorder = SegmentRecorder(config: config, format: format, directory: directory)
        post(.connected(hasTitles: true, format: format))
        completionHandler(.allow)
    }

    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
        guard !stopped, var parser, let recorder else { return }
        var events: [SegmentRecorder.Event] = []
        for e in parser.feed(data) {
            switch e {
            case .audio(let d): events += recorder.handleAudio(d)
            case .metadata(let m):
                if let title = ICYParser.streamTitle(in: m) { events += recorder.handleTitle(title) }
            }
        }
        self.parser = parser
        if !events.isEmpty { post(.recorder(events)) }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        guard !stopped else { return }
        stopped = true
        if let events = recorder?.streamEnded(), !events.isEmpty { post(.recorder(events)) }
        post(.ended(error?.localizedDescription))
        session.finishTasksAndInvalidate()
    }
}
