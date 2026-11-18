import AppKit
import SwiftUI

/// The Windows app's look (WinUI 3 / Fluent), drawn with Mac controls: the shared colours, type
/// sizes and building blocks. Values are WinUI's default theme resources (taken from the
/// Windows app's XAML); docs/MAC-WINDOWS-SYNC.md tracks comparing them with the Windows renders.
enum Fluent {
    /// A colour with separate light and dark values.
    static func dynamic(light: NSColor, dark: NSColor) -> Color {
        Color(nsColor: NSColor(name: nil) { $0.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua ? dark : light })
    }

    static func white(_ a: CGFloat) -> NSColor { NSColor(white: 1, alpha: a) }
    static func black(_ a: CGFloat) -> NSColor { NSColor(white: 0, alpha: a) }

    /// Mica: the window background under the title bar and navigation pane.
    static let mica = dynamic(light: NSColor(srgbRed: 0.953, green: 0.953, blue: 0.953, alpha: 1),
                              dark: NSColor(srgbRed: 0.125, green: 0.125, blue: 0.125, alpha: 1))
    /// LayerFillColorDefault: the content area that sits on Mica.
    static let layer = dynamic(light: white(0.5), dark: NSColor(srgbRed: 0.227, green: 0.227, blue: 0.227, alpha: 0.3))
    /// LayerFillColorAlt: the bottom mini player bar.
    static let layerAlt = dynamic(light: white(1), dark: white(0.05))
    /// CardBackgroundFillColorDefault.
    static let card = dynamic(light: white(0.7), dark: white(0.05))
    /// CardStrokeColorDefault.
    static let cardStroke = dynamic(light: black(0.06), dark: black(0.1))
    /// The content layer's edge (a little stronger than a card's so it reads on Mica).
    static let layerStroke = dynamic(light: black(0.08), dark: black(0.25))
    /// SubtleFillColorSecondary / Tertiary: hover and selection behind nav items and cards.
    static let subtle = dynamic(light: black(0.037), dark: white(0.061))
    static let subtleHover = dynamic(light: black(0.024), dark: white(0.042))
    /// DividerStrokeColorDefault.
    static let divider = dynamic(light: black(0.08), dark: white(0.08))

    /// TextFillColorTertiary: empty-state icons and chevrons.
    static let tertiaryText = dynamic(light: black(0.447), dark: white(0.53))
    /// AccentFillColorDefault: the accent, but the lighter shade in dark mode (WinUI's rule), so
    /// glyphs on it are white in light mode and black in dark mode.
    static let accentFill = Color(nsColor: NSColor(name: nil) { a in
        let accent = NSColor.controlAccentColor.usingColorSpace(.sRGB) ?? .systemBlue
        return a.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua ? accent.blended(withFraction: 0.45, of: .white) ?? accent : accent
    })
    static let onAccent = dynamic(light: .white, dark: .black)

    static let titleFont = Font.system(size: 28, weight: .semibold)
    static let subtitleFont = Font.system(size: 20, weight: .semibold)
}

/// A page's header: the large title on the left, its controls on the right.
struct PageHeader<Trailing: View>: View {
    let title: String
    @ViewBuilder var trailing: Trailing

    var body: some View {
        HStack(spacing: 16) {
            Text(title).font(Fluent.titleFont).lineLimit(1).fixedSize()
            Spacer(minLength: 0)
            HStack(spacing: 6) { trailing }.controlSize(.large)
        }
        .frame(minHeight: 40)
        .padding(EdgeInsets(top: 12, leading: 24, bottom: 8, trailing: 16))
    }
}

extension PageHeader where Trailing == EmptyView {
    init(title: String) { self.init(title: title) { EmptyView() } }
}

/// A Fluent card: a light fill with a hairline edge and 8 pt corners.
struct FluentCard<Content: View>: View {
    var padding = EdgeInsets(top: 12, leading: 16, bottom: 12, trailing: 16)
    @ViewBuilder var content: Content

    var body: some View {
        content
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(padding)
            .background(RoundedRectangle(cornerRadius: 8, style: .continuous).fill(Fluent.card))
            .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).strokeBorder(Fluent.cardStroke, lineWidth: 1))
    }
}

/// A semibold label above a group of cards.
struct GroupTitle: View {
    let text: String
    var top: CGFloat = 20
    var body: some View {
        Text(text).fontWeight(.semibold).padding(.leading, 2).padding(.top, top).padding(.bottom, 4)
            .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// A big icon, a title, a line of text and optional buttons, centred in a page.
struct EmptyState<Actions: View>: View {
    @Environment(\.colorScheme) private var scheme
    let symbol: String
    let title: String
    var text: String = ""
    var iconSize: CGFloat = 40
    @ViewBuilder var actions: Actions

    var body: some View {
        VStack(spacing: 10) {
            Image(systemName: symbol).font(.system(size: iconSize, weight: .light)).foregroundStyle(scheme == .dark ? Color.white.opacity(0.53) : Color.black.opacity(0.447))
                .padding(.bottom, 2)
            Text(title).font(Fluent.subtitleFont).multilineTextAlignment(.center)
            if !text.isEmpty {
                Text(text).foregroundStyle(.secondary).multilineTextAlignment(.center)
            }
            HStack(spacing: 8) { actions }.controlSize(.large).padding(.top, 6)
        }
        .frame(maxWidth: 420)
        .padding(24)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

extension EmptyState where Actions == EmptyView {
    init(symbol: String, title: String, text: String = "", iconSize: CGFloat = 40) {
        self.init(symbol: symbol, title: title, text: text, iconSize: iconSize) { EmptyView() }
    }
}

/// A navigation pane item: icon and label, a subtle fill when selected or hovered, and the
/// accent pill on the left of the selected one.
struct NavRow: View {
    let title: String
    let symbol: String
    var selected = false
    var compact = false
    var help: String?
    let action: () -> Void
    @ViewState private var hovering = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: 14) {
                Image(systemName: symbol).font(.system(size: 15)).frame(width: 20)
                if !compact { Text(title).lineLimit(1) }
            }
            .frame(maxWidth: .infinity, alignment: compact ? .center : .leading)
            .padding(.horizontal, compact ? 0 : 10)
            .frame(height: 36)
            .contentShape(Rectangle())
            .background(RoundedRectangle(cornerRadius: 5, style: .continuous)
                .fill(selected ? Fluent.subtle : hovering && !Diagnostics.rendering ? Fluent.subtleHover : .clear))
            .overlay(alignment: .leading) {
                if selected {
                    Capsule().fill(.tint).frame(width: 3, height: 16)
                }
            }
        }
        .buttonStyle(.plain)
        .onHover { hovering = $0 }
        .help(help ?? title)
        .accessibilityLabel(title)
        .accessibilityAddTraits(selected ? .isSelected : [])
    }
}

/// WinUI's accent button: the accent fill (its lighter shade in dark mode) with white text in light
/// mode and black text in dark mode, 4 pt corners, 32 pt tall at the large size.
struct AccentButtonStyle: ButtonStyle {
    @Environment(\.isEnabled) private var enabled
    @Environment(\.controlSize) private var size

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .lineLimit(1)
            .fixedSize()
            .fontWeight(.medium)
            .foregroundStyle(enabled ? AnyShapeStyle(Fluent.onAccent) : AnyShapeStyle(.tertiary))
            .padding(.horizontal, 12)
            .frame(minHeight: size == .large ? 32 : 24)
            .background(RoundedRectangle(cornerRadius: 4, style: .continuous)
                .fill(enabled ? AnyShapeStyle(Fluent.accentFill) : AnyShapeStyle(.quaternary)))
            .opacity(configuration.isPressed ? 0.8 : 1)
            .contentShape(Rectangle())
    }
}

extension ButtonStyle where Self == AccentButtonStyle {
    static var accent: AccentButtonStyle { AccentButtonStyle() }
}

/// A neutral drop-down button (WinUI DropDownButton): an icon, an optional title and a chevron, no
/// coloured arrow. macOS menus keep only one image from a label, so it's all one Text.
struct DropDownMenu<Content: View>: View {
    let symbol: String
    var title: String = ""
    @ViewBuilder var content: Content
    var body: some View {
        Menu {
            content
        } label: {
            Text("\(Image(systemName: symbol))\(title.isEmpty ? "" : "  " + title)  \(Image(systemName: "chevron.down"))")
        }
        .menuStyle(.button)
        .buttonStyle(.bordered)
        .menuIndicator(.hidden)
        .fixedSize()
    }
}
