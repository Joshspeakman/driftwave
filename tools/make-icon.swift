// Draws the app icon (1024x1024 PNG): swift tools/make-icon.swift out.png
import AppKit

let size = 1024.0
let out = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "icon.png"
let cs = CGColorSpace(name: CGColorSpace.sRGB)!
let ctx = CGContext(data: nil, width: Int(size), height: Int(size), bitsPerComponent: 8, bytesPerRow: 0,
                    space: cs, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!

func color(_ r: Double, _ g: Double, _ b: Double, _ a: Double = 1) -> CGColor { CGColor(colorSpace: cs, components: [r, g, b, a])! }

// macOS icon grid: 824pt rounded square centred in 1024 with ~185 corner radius.
let inset = 100.0
let body = CGRect(x: inset, y: inset, width: size - 2 * inset, height: size - 2 * inset)
let shape = CGPath(roundedRect: body, cornerWidth: 185, cornerHeight: 185, transform: nil)

// Drop shadow
ctx.saveGState()
ctx.setShadow(offset: CGSize(width: 0, height: -12), blur: 28, color: color(0, 0, 0, 0.35))
ctx.addPath(shape); ctx.setFillColor(color(0.1, 0.1, 0.2)); ctx.fillPath()
ctx.restoreGState()

// Background gradient: dusk orange to deep night blue
ctx.saveGState()
ctx.addPath(shape); ctx.clip()
let bg = CGGradient(colorsSpace: cs, colors: [color(0.99, 0.55, 0.30), color(0.80, 0.25, 0.42), color(0.18, 0.15, 0.42)] as CFArray,
                    locations: [0, 0.45, 1])!
ctx.drawLinearGradient(bg, start: CGPoint(x: size / 2, y: body.maxY), end: CGPoint(x: size / 2, y: body.minY), options: [])

// Globe-ish horizon at the bottom
let horizon = CGRect(x: body.midX - 560, y: body.minY - 820, width: 1120, height: 1120)
ctx.setFillColor(color(0.08, 0.07, 0.22, 0.85))
ctx.fillEllipse(in: horizon)
ctx.setStrokeColor(color(1, 1, 1, 0.10)); ctx.setLineWidth(6)
for i in 1...3 {
    let d = Double(i) * 140
    ctx.strokeEllipse(in: horizon.insetBy(dx: d, dy: 0))
}

// Antenna mast + waves
let tip = CGPoint(x: body.midX, y: body.minY + 420)
ctx.setStrokeColor(color(1, 1, 1)); ctx.setLineCap(.round)
ctx.setLineWidth(30)
ctx.move(to: CGPoint(x: tip.x, y: tip.y - 30)); ctx.addLine(to: CGPoint(x: tip.x - 95, y: body.minY + 230))
ctx.move(to: CGPoint(x: tip.x, y: tip.y - 30)); ctx.addLine(to: CGPoint(x: tip.x + 95, y: body.minY + 230))
ctx.strokePath()
ctx.setFillColor(color(1, 1, 1))
ctx.fillEllipse(in: CGRect(x: tip.x - 42, y: tip.y - 42, width: 84, height: 84))
for (i, r) in [130.0, 215.0, 300.0].enumerated() {
    ctx.setStrokeColor(color(1, 1, 1, 1 - Double(i) * 0.22))
    ctx.setLineWidth(34)
    ctx.addArc(center: tip, radius: r, startAngle: .pi * 0.22, endAngle: .pi * 0.78, clockwise: false)
    ctx.strokePath()
}
ctx.restoreGState()

let img = ctx.makeImage()!
let rep = NSBitmapImageRep(cgImage: img)
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: out))
