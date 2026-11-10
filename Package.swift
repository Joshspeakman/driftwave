// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "Shortwave",
    platforms: [.macOS(.v15)],
    targets: [
        .target(name: "ShortwaveCore", path: "Sources/ShortwaveCore"),
        .executableTarget(name: "Shortwave", dependencies: ["ShortwaveCore"], path: "Sources/Shortwave"),
        .testTarget(name: "ShortwaveTests", dependencies: ["ShortwaveCore"], path: "Tests/ShortwaveTests"),
    ],
    swiftLanguageModes: [.v5]
)
