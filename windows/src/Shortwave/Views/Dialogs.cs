using System.Globalization;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Documents;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Shortwave.Core;
using Shortwave.Model;

namespace Shortwave.Views;

/// <summary>Builders shared by the dialogs: grouped sections with label/value rows, like the Mac's grouped forms.</summary>
static class Form
{
    public static Brush Res(string key) => Ui.ThemeBrush(key);

    public static Border Section(string? title, StackPanel host, params UIElement[] rows)
    {
        var list = new StackPanel { Spacing = 0 };
        bool first = true;
        foreach (var r in rows)
        {
            if (!first) list.Children.Add(new Border { Height = 1, Background = Res("DividerStrokeColorDefaultBrush"), Margin = new Thickness(0, 6, 0, 6) });
            list.Children.Add(r);
            first = false;
        }
        var card = new Border
        {
            Background = Res("CardBackgroundFillColorDefaultBrush"),
            BorderBrush = Res("CardStrokeColorDefaultBrush"),
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(8),
            Padding = new Thickness(14, 10, 14, 10),
            Child = list,
        };
        if (rows.Length == 0) return card;
        if (title != null) host.Children.Add(new TextBlock { Text = title, FontWeight = FontWeights.SemiBold, Margin = new Thickness(2, 10, 0, 4) });
        host.Children.Add(card);
        return card;
    }

    public static UIElement Row(string label, UIElement value)
    {
        var g = new Grid { ColumnSpacing = 16 };
        g.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        g.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        g.Children.Add(new TextBlock { Text = label, VerticalAlignment = VerticalAlignment.Center });
        if (value is FrameworkElement fe) { fe.HorizontalAlignment = HorizontalAlignment.Right; fe.VerticalAlignment = VerticalAlignment.Center; }
        Grid.SetColumn((FrameworkElement)value, 1);
        g.Children.Add(value);
        return g;
    }

    public static TextBlock Value(string text) => new()
    {
        Text = text,
        Foreground = Res("TextFillColorSecondaryBrush"),
        TextWrapping = TextWrapping.Wrap,
        TextAlignment = TextAlignment.Right,
        IsTextSelectionEnabled = true,
    };

    public static UIElement? TextRow(string label, string value) => value.Length == 0 ? null : Row(label, Value(value));

    public static UIElement Note(string glyph, string title, string text)
    {
        var p = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 10 };
        p.Children.Add(new FontIcon { Glyph = glyph, FontSize = 16, VerticalAlignment = VerticalAlignment.Top, Margin = new Thickness(0, 2, 0, 0) });
        var t = new StackPanel { Spacing = 2, MaxWidth = 420 };
        t.Children.Add(new TextBlock { Text = title, FontWeight = FontWeights.SemiBold });
        t.Children.Add(new TextBlock { Text = text, Foreground = Res("TextFillColorSecondaryBrush"), TextWrapping = TextWrapping.Wrap });
        p.Children.Add(t);
        return p;
    }

    public static UIElement[] Rows(params UIElement?[] rows) => rows.OfType<UIElement>().ToArray();

    public static Button IconButton(string glyph, string tip, Action click)
    {
        var b = new Button { Content = new FontIcon { Glyph = glyph, FontSize = 14 }, Padding = new Thickness(6, 4, 6, 4), Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent), BorderThickness = new Thickness(0) };
        ToolTipService.SetToolTip(b, tip);
        AutomationProperties.SetName(b, tip);
        b.Click += (_, _) => click();
        return b;
    }

    public static Button LabeledButton(string glyph, string text, Action click, bool accent = false)
    {
        var sp = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
        sp.Children.Add(new FontIcon { Glyph = glyph, FontSize = 14 });
        sp.Children.Add(new TextBlock { Text = text });
        var b = new Button { Content = sp };
        if (accent) b.Style = (Style)Application.Current.Resources["AccentButtonStyle"];
        AutomationProperties.SetName(b, text);
        b.Click += (_, _) => click();
        return b;
    }
}

/// <summary>Station details: cover, tags, play, library, information, location and audio.</summary>
public sealed class StationDetailDialog : ContentDialog
{
    private readonly AppModel app;
    private Station station;
    private readonly Func<nint> hwnd;

    public StationDetailDialog(AppModel app, Station station, Func<nint> hwnd)
    {
        this.app = app; this.station = station; this.hwnd = hwnd;
        CloseButtonText = "Close";
        DefaultButton = ContentDialogButton.None;
        Resources["ContentDialogMaxWidth"] = 640.0;
        Build();
        app.Player.PropertyChanged += OnPlayer;
        Closed += (_, _) => app.Player.PropertyChanged -= OnPlayer;
    }

    void OnPlayer(object? s, System.ComponentModel.PropertyChangedEventArgs e)
    {
        if (e.PropertyName is nameof(Audio.Player.State) or nameof(Audio.Player.Station) or "") Build();
    }

    bool IsActive => app.Player.Station?.Id == station.Id && app.Player.IsPlaying;

    void Build()
    {
        bool inLibrary = app.IsInLibrary(station);
        var root = new StackPanel { Spacing = 4, Width = 540 };

        // Header
        var header = new Grid { ColumnSpacing = 16, Margin = new Thickness(0, 0, 0, 8) };
        header.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        header.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        header.Children.Add(new StationCover { Size = 112, Station = station, VerticalAlignment = VerticalAlignment.Top });
        var info = new StackPanel { Spacing = 6 };
        Grid.SetColumn(info, 1);
        info.Children.Add(new TextBlock { Text = station.Name, Style = (Style)Application.Current.Resources["SubtitleTextBlockStyle"], TextWrapping = TextWrapping.Wrap, IsTextSelectionEnabled = true });
        if (station.Tags.Count > 0)
            info.Children.Add(new TextBlock { Text = string.Join(" · ", station.Tags.Take(8)), Foreground = Form.Res("TextFillColorSecondaryBrush"), TextWrapping = TextWrapping.Wrap, MaxLines = 2, TextTrimming = TextTrimming.CharacterEllipsis });
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, Margin = new Thickness(0, 4, 0, 0) };
        var play = Form.LabeledButton("\uF5B0", IsActive ? "Playing" : "Play", () => app.Play(station), accent: true);
        play.IsEnabled = !IsActive;
        buttons.Children.Add(play);
        buttons.Children.Add(Form.LabeledButton(inLibrary ? "\uE738" : "\uE710", inLibrary ? "Remove from Library" : "Add to Library", () =>
        {
            app.ToggleLibrary(station);
            station = app.LibraryVersion(station);
            Build();
        }));
        if (inLibrary)
        {
            var more = new DropDownButton { Content = new FontIcon { Glyph = "\uE712", FontSize = 14 } };
            AutomationProperties.SetName(more, "More");
            var menu = new MenuFlyout();
            if (station.IsLocal) menu.Items.Add(MenuItem("Edit Station…", () => { Hide(); app.EditStation(station); }));
            menu.Items.Add(MenuItem("Change Station Cover…", async () =>
            {
                if (await Ui.PickImage(hwnd()) is { } path) { await app.SetCover(station, path); station = app.LibraryVersion(station); Build(); }
            }));
            if (station.CustomCover != null)
                menu.Items.Add(MenuItem("Use Default Cover", async () => { await app.SetCover(station, null); station = app.LibraryVersion(station); Build(); }));
            more.Flyout = menu;
            buttons.Children.Add(more);
        }
        info.Children.Add(buttons);
        header.Children.Add(info);
        root.Children.Add(header);

        if (station.IsLocal)
            Form.Section(null, root, Form.Note("\uE946", "Local Station", "This station exists only in your library, and is not part of the public database."));
        else if (station.IsOrphaned)
            Form.Section(null, root, Form.Note("\uE946", "Orphaned Station", "The information could not be updated, probably this station was removed from the public database."));

        UIElement? homepage = null;
        if (station.Homepage is { } home && Uri.TryCreate(home, UriKind.Absolute, out var homeUrl) && (homeUrl.Scheme == "http" || homeUrl.Scheme == "https"))
            homepage = Form.Row("Homepage", new HyperlinkButton { Content = homeUrl.Host, NavigateUri = homeUrl, Padding = new Thickness(0) });
        Form.Section("Information", root, Form.Rows(
            Form.TextRow("Language", station.Language),
            Form.TextRow("Tags", string.Join(", ", station.Tags)),
            station.IsLocal ? null : Form.TextRow("Votes", station.Votes > 0 ? station.Votes.ToString("N0", CultureInfo.CurrentCulture) : ""),
            homepage));

        if (station.Country.Length > 0 || station.State.Length > 0 || station.HasLocation)
        {
            UIElement? map = null;
            if (station is { Latitude: { } lat, Longitude: { } lon })
            {
                // WinUI's map control needs an Azure Maps key; open the location on OpenStreetMap instead.
                string coords = $"{lat.ToString("0.####", CultureInfo.InvariantCulture)}, {lon.ToString("0.####", CultureInfo.InvariantCulture)}";
                var url = new Uri($"https://www.openstreetmap.org/?mlat={lat.ToString(CultureInfo.InvariantCulture)}&mlon={lon.ToString(CultureInfo.InvariantCulture)}#map=11/{lat.ToString(CultureInfo.InvariantCulture)}/{lon.ToString(CultureInfo.InvariantCulture)}");
                var sp = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
                sp.Children.Add(Form.Value(coords));
                sp.Children.Add(new HyperlinkButton { Content = "Open Map", NavigateUri = url, Padding = new Thickness(0) });
                map = Form.Row("Coordinates", sp);
            }
            Form.Section("Location", root, Form.Rows(Form.TextRow("Country", station.Country), Form.TextRow("State", station.State), map));
        }

        var stream = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 4 };
        stream.Children.Add(new TextBlock { Text = station.StreamURL, MaxWidth = 340, TextTrimming = TextTrimming.CharacterEllipsis, IsTextSelectionEnabled = true, Foreground = Form.Res("TextFillColorSecondaryBrush"), VerticalAlignment = VerticalAlignment.Center });
        stream.Children.Add(Form.IconButton("\uE8C8", "Copy Stream URL", () => Ui.Copy(station.StreamURL)));
        ToolTipService.SetToolTip(stream, station.StreamURL);
        Form.Section("Audio", root, Form.Rows(
            Form.TextRow("Codec", station.Codec + (station.IsHLS ? " (HLS)" : "")),
            Form.TextRow("Bitrate", station.Bitrate > 0 ? $"{station.Bitrate} kbps" : ""),
            Form.Row("Stream", stream)));

        Content = new ScrollViewer { Content = root, MaxHeight = 560, Padding = new Thickness(0, 0, 12, 0) };
    }

    static MenuFlyoutItem MenuItem(string text, Action action)
    {
        var i = new MenuFlyoutItem { Text = text };
        i.Click += (_, _) => action();
        return i;
    }
}

/// <summary>Add a local station, or edit one.</summary>
public sealed class LocalStationDialog : ContentDialog
{
    private readonly AppModel app;
    private readonly Station? editing;
    private readonly Func<nint> hwnd;
    private readonly TextBox name = new() { Header = "Name", PlaceholderText = "Station name" };
    private readonly TextBox url = new() { Header = "Stream URL", PlaceholderText = "https://example.com/stream.mp3", InputScope = new Microsoft.UI.Xaml.Input.InputScope { Names = { new Microsoft.UI.Xaml.Input.InputScopeName(Microsoft.UI.Xaml.Input.InputScopeNameValue.Url) } } };
    private readonly Image preview = new() { Width = 32, Height = 32, Visibility = Visibility.Collapsed };
    private readonly Button choose = new() { Content = "Choose…" };
    private string? coverPath;

    public LocalStationDialog(AppModel app, Station? editing, Func<nint> hwnd)
    {
        this.app = app; this.editing = editing; this.hwnd = hwnd;
        Title = editing == null ? "Add Local Station" : "Edit Station";
        PrimaryButtonText = editing == null ? "Add Station" : "Save";
        CloseButtonText = "Cancel";
        DefaultButton = ContentDialogButton.Primary;
        if (editing != null) { name.Text = editing.Name; url.Text = editing.StreamURL; }

        var root = new StackPanel { Spacing = 12, Width = 420 };
        root.Children.Add(name);
        root.Children.Add(url);
        var cover = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 10 };
        cover.Children.Add(new TextBlock { Text = "Cover", VerticalAlignment = VerticalAlignment.Center, MinWidth = 60 });
        cover.Children.Add(preview);
        cover.Children.Add(choose);
        choose.Click += async (_, _) =>
        {
            if (await Ui.PickImage(hwnd()) is { } p)
            {
                coverPath = p;
                preview.Source = new BitmapImage(new Uri(p)) { DecodePixelWidth = 64 };
                preview.Visibility = Visibility.Visible;
                choose.Content = "Change…";
            }
        };
        root.Children.Add(cover);
        root.Children.Add(new TextBlock { Text = "Playlist links (.pls, .m3u) are followed automatically.", Foreground = Form.Res("TextFillColorSecondaryBrush"), TextWrapping = TextWrapping.Wrap });
        if (editing == null)
        {
            var pub = new StackPanel { Spacing = 2, Margin = new Thickness(0, 8, 0, 0) };
            pub.Children.Add(new HyperlinkButton { Content = "Add a public station to radio-browser.info", NavigateUri = new Uri("https://www.radio-browser.info/add"), Padding = new Thickness(0) });
            pub.Children.Add(new TextBlock { Text = "Public stations are added to the community database and become available for everyone.", Foreground = Form.Res("TextFillColorSecondaryBrush"), TextWrapping = TextWrapping.Wrap, Style = (Style)Application.Current.Resources["CaptionTextBlockStyle"] });
            root.Children.Add(pub);
        }
        Content = root;
        name.TextChanged += (_, _) => Validate();
        url.TextChanged += (_, _) => Validate();
        Validate();
        PrimaryButtonClick += async (_, e) =>
        {
            var d = e.GetDeferral();
            try { await Commit(); } finally { d.Complete(); }
        };
    }

    bool IsValid =>
        name.Text.Trim().Length > 0
        && Uri.TryCreate(url.Text.Trim(), UriKind.Absolute, out var u) && (u.Scheme == "http" || u.Scheme == "https") && u.Host.Length > 0;

    void Validate() => IsPrimaryButtonEnabled = IsValid;

    async Task Commit()
    {
        string cleanName = name.Text.Trim(), cleanURL = url.Text.Trim();
        if (editing is { } e)
        {
            var s = app.LibraryVersion(e).Copy();
            s.Name = cleanName;
            s.StreamURL = cleanURL;
            app.Update(s);
            if (coverPath != null) await app.SetCover(s, coverPath);
        }
        else
        {
            var s = Station.Local(cleanName, cleanURL);
            app.AddToLibrary(s);
            if (coverPath != null) await app.SetCover(s, coverPath);
        }
    }
}

/// <summary>Details of a detected track: recording state, duration, save, play, cancel.</summary>
public sealed class TrackDetailDialog : ContentDialog
{
    private readonly AppModel app;
    private readonly Track track;
    private readonly DispatcherTimer timer = new() { Interval = TimeSpan.FromSeconds(1) };
    private TextBlock? durationText;

    public TrackDetailDialog(AppModel app, Track track)
    {
        this.app = app; this.track = track;
        CloseButtonText = "Close";
        DefaultButton = ContentDialogButton.Close;
        Resources["ContentDialogMaxWidth"] = 560.0;
        PrimaryButtonClick += async (_, e) =>
        {
            e.Cancel = true;    // saving keeps the dialog open, like the Mac sheet
            await app.Player.Save(track);
        };
        SecondaryButtonClick += (_, e) =>
        {
            e.Cancel = true;
            if (track.SavedTo is { } saved) Ui.Open(saved);
        };
        track.PropertyChanged += OnTrackChanged;
        app.Player.PropertyChanged += OnPlayerChanged;
        timer.Tick += (_, _) => UpdateDuration();
        Closed += (_, _) => { track.PropertyChanged -= OnTrackChanged; app.Player.PropertyChanged -= OnPlayerChanged; timer.Stop(); };
        Build();
        timer.Start();
    }

    // Rebuild only for changes the dialog shows (not on every buffering blip, which would also reset the checkbox).
    void OnTrackChanged(object? s, System.ComponentModel.PropertyChangedEventArgs e)
    {
        if (e.PropertyName is nameof(Track.State) or nameof(Track.SavedTo) or nameof(Track.SaveError) or nameof(Track.File) or "") Build();
        else if (e.PropertyName is nameof(Track.Duration)) UpdateDuration();
    }

    void OnPlayerChanged(object? s, System.ComponentModel.PropertyChangedEventArgs e)
    {
        if (e.PropertyName is nameof(Audio.Player.PlayingTrack) or "") Build();
    }

    void UpdateDuration()
    {
        if (durationText is null) return;
        if (track.State == RecordingState.Recording)
        {
            int s = (int)(DateTime.Now - track.StartedAt).TotalSeconds;
            durationText.Text = $"{s / 60}:{s % 60:00}";
        }
        else durationText.Text = track.DurationText;
    }

    void Build()
    {
        var root = new StackPanel { Spacing = 4, Width = 440 };
        var header = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 14, Margin = new Thickness(0, 0, 0, 8) };
        header.Children.Add(new StationCover { Size = 64, Station = track.Station });
        var names = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center, MaxWidth = 340 };
        names.Children.Add(new TextBlock { Text = track.SongTitle, Style = (Style)Application.Current.Resources["SubtitleTextBlockStyle"], TextWrapping = TextWrapping.Wrap, IsTextSelectionEnabled = true });
        if (track.Artist is { } artist) names.Children.Add(new TextBlock { Text = artist, IsTextSelectionEnabled = true, TextWrapping = TextWrapping.Wrap });
        names.Children.Add(new TextBlock { Text = track.Station.Name, Foreground = Form.Res("TextFillColorSecondaryBrush") });
        header.Children.Add(names);
        root.Children.Add(header);

        var rows = new List<UIElement?>
        {
            Form.Row("State", Form.Value(track.StateTitle)),
            new TextBlock { Text = track.StateExplanation, Foreground = Form.Res("TextFillColorSecondaryBrush"), TextWrapping = TextWrapping.Wrap },
        };
        durationText = null;
        if (track.State.IsRecorded() || track.State == RecordingState.Recording)
        {
            durationText = Form.Value("");
            rows.Add(Form.Row("Duration", durationText));
            UpdateDuration();
        }
        if (track.State == RecordingState.Recording && app.Settings.RecordingMode == RecordingMode.Decide)
        {
            var box = new CheckBox { Content = "Save this track when recording is complete", IsChecked = track.SaveWhenRecorded };
            box.Click += (_, _) => track.SaveWhenRecorded = box.IsChecked == true;
            rows.Add(box);
        }
        if (track.SavedTo is { } saved)
        {
            var link = new HyperlinkButton { Content = Path.GetFileName(saved), Padding = new Thickness(0) };
            ToolTipService.SetToolTip(link, saved);
            link.Click += (_, _) => Ui.Reveal(saved);
            rows.Add(Form.Row("Saved", link));
        }
        if (track.SaveError is { } err)
        {
            var e = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
            e.Children.Add(new FontIcon { Glyph = "\uE7BA", FontSize = 14, Foreground = Form.Res("SystemFillColorCautionBrush") });
            e.Children.Add(new TextBlock { Text = err, Foreground = Form.Res("SystemFillColorCautionBrush"), TextWrapping = TextWrapping.Wrap, MaxWidth = 380 });
            rows.Add(e);
        }
        if (track.State == RecordingState.Recording && app.Player.PlayingTrack?.Id == track.Id)
        {
            var cancel = new Button { Content = "Cancel Recording", Foreground = Form.Res("SystemFillColorCriticalBrush") };
            cancel.Click += (_, _) => app.Player.CancelRecording();
            rows.Add(cancel);
        }
        Form.Section("Recording", root, Form.Rows([.. rows]));

        var about = new StackPanel { Spacing = 6 };
        about.Children.Add(new TextBlock { Text = "Tracks are recognised from the station’s stream metadata. Recording behaviour can be changed in Settings.", Foreground = Form.Res("TextFillColorSecondaryBrush"), TextWrapping = TextWrapping.Wrap });
        var open = new HyperlinkButton { Content = "Open Settings…", Padding = new Thickness(0) };
        open.Click += (_, _) => { Hide(); App.Current?.ShowSettings(); };
        about.Children.Add(open);
        Form.Section(null, root, about);

        Content = root;
        PrimaryButtonText = track.CanSave ? "Save Track" : "";
        SecondaryButtonText = track.SavedTo != null ? "Play Track" : "";
        DefaultButton = track.CanSave ? ContentDialogButton.Primary : ContentDialogButton.Close;
    }
}
