using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Imaging;
using Shortwave.Model;

namespace Shortwave.Views;

/// <summary>Settings > Appearance > Background: your own GIF or picture behind the window, from a file or GIPHY.</summary>
public sealed class BackgroundSettings : StackPanel
{
    readonly AppModel app;
    readonly Func<nint> hwnd;
    readonly Image preview = new() { Width = 128, Height = 72, Stretch = Microsoft.UI.Xaml.Media.Stretch.UniformToFill };
    readonly TextBlock status = new() { TextWrapping = TextWrapping.Wrap, Visibility = Visibility.Collapsed };
    readonly Button remove = new() { Content = "Remove" };
    readonly Slider dim = new() { Minimum = 0, Maximum = 80, Width = 200, StepFrequency = 5, Header = "Dim" };
    readonly TextBox key = new() { PlaceholderText = "GIPHY API key", Width = 280 };
    bool updating;

    public BackgroundSettings(AppModel app, Func<nint> hwnd, Style cardStyle)
    {
        this.app = app;
        this.hwnd = hwnd;
        var stack = new StackPanel { Spacing = 10 };
        stack.Children.Add(new TextBlock { Text = "Background" });
        stack.Children.Add(new TextBlock
        {
            Text = "Your own GIF or picture behind the whole window, in any theme. GIFs loop, and rest while Driftwave isn't the active window. Anything that flashes is refused.",
            TextWrapping = TextWrapping.Wrap, Style = (Style)Application.Current.Resources["CaptionSecondaryStyle"],
        });
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12 };
        row.Children.Add(new Border { CornerRadius = new CornerRadius(6), Child = preview, Width = 128, Height = 72,
            Background = Ui.ThemeBrush("ControlFillColorSecondaryBrush") });
        var buttons = new StackPanel { Spacing = 8, VerticalAlignment = VerticalAlignment.Center };
        var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        var choose = new Button { Content = "Choose File…" };
        var search = new Button { Content = "Search GIPHY…" };
        line.Children.Add(choose);
        line.Children.Add(search);
        line.Children.Add(remove);
        buttons.Children.Add(line);
        buttons.Children.Add(dim);
        row.Children.Add(buttons);
        stack.Children.Add(row);
        stack.Children.Add(status);
        var keyRow = new Expander { Header = "GIPHY key", HorizontalAlignment = HorizontalAlignment.Stretch, HorizontalContentAlignment = HorizontalAlignment.Left };
        var keyStack = new StackPanel { Spacing = 6 };
        keyStack.Children.Add(new TextBlock { Text = "Searching GIPHY needs a free API key from developers.giphy.com. Paste yours here (it stays on this PC).",
            TextWrapping = TextWrapping.Wrap, Style = (Style)Application.Current.Resources["CaptionSecondaryStyle"] });
        keyStack.Children.Add(key);
        keyRow.Content = keyStack;
        stack.Children.Add(keyRow);
        Children.Add(new Border { Style = cardStyle, Child = stack });

        choose.Click += async (_, _) =>
        {
            if (await Ui.PickImage(hwnd()) is { } file) Show(await app.Backgrounds.UseFile(file));
        };
        search.Click += async (_, _) => await SearchGiphy();
        remove.Click += (_, _) => { app.Backgrounds.Clear(); Show(null); };
        dim.ValueChanged += (_, e) => { if (!updating) app.Settings.BackgroundDim = e.NewValue / 100; };
        key.LostFocus += (_, _) => { if (!updating) app.Settings.GiphyKey = key.Text.Trim(); };
        app.Settings.PropertyChanged += (_, e) => { if (e.PropertyName is nameof(Preferences.BackgroundFile)) DispatcherQueue.TryEnqueue(Refresh); };
        Refresh();
    }

    void Show(string? problem)
    {
        status.Text = problem ?? "";
        status.Visibility = problem is null ? Visibility.Collapsed : Visibility.Visible;
        Refresh();
    }

    void Refresh()
    {
        updating = true;
        var path = app.Backgrounds.CurrentPath;
        preview.Source = path is null ? null : new BitmapImage(new Uri(path)) { DecodePixelWidth = 256, AutoPlay = false };
        remove.IsEnabled = path != null;
        dim.Value = Math.Round(app.Settings.BackgroundDim * 100);
        key.Text = app.Settings.GiphyKey ?? "";
        updating = false;
    }

    /// <summary>A GIPHY search dialog: type, pick, and it's checked and used.</summary>
    async Task SearchGiphy()
    {
        var box = new TextBox { PlaceholderText = "Search GIPHY (rated G)", Width = 420 };
        var go = new Button { Content = "Search" };
        var results = new GridView { SelectionMode = ListViewSelectionMode.None, IsItemClickEnabled = true, Height = 380 };
        var note = new TextBlock { TextWrapping = TextWrapping.Wrap, Style = (Style)Application.Current.Resources["CaptionSecondaryStyle"] };
        var top = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        top.Children.Add(box);
        top.Children.Add(go);
        var body = new StackPanel { Spacing = 10, Width = 560 };
        body.Children.Add(top);
        body.Children.Add(note);
        body.Children.Add(results);
        body.Children.Add(new TextBlock { Text = "Powered by GIPHY", Style = (Style)Application.Current.Resources["CaptionSecondaryStyle"] });
        var dialog = new ContentDialog { Title = "Search GIPHY", Content = body, CloseButtonText = "Cancel", XamlRoot = XamlRoot };
        async Task Run()
        {
            if (box.Text.Trim().Length == 0) return;
            note.Text = "Searching…";
            try
            {
                var found = await app.Backgrounds.Search(box.Text.Trim());
                results.Items.Clear();
                foreach (var r in found)
                {
                    var img = new Image { Width = 160, Height = 100, Stretch = Microsoft.UI.Xaml.Media.Stretch.UniformToFill,
                        Source = new BitmapImage(new Uri(r.Preview)) };
                    ToolTipService.SetToolTip(img, r.Title);
                    results.Items.Add(new Border { CornerRadius = new CornerRadius(6), Child = img, Tag = r });
                }
                note.Text = found.Count == 0 ? "Nothing found." : "Click one to use it. It's checked for flashing first.";
            }
            catch (Exception e) { note.Text = e.Message; }
        }
        go.Click += async (_, _) => await Run();
        box.KeyDown += async (_, e) => { if (e.Key == Windows.System.VirtualKey.Enter) await Run(); };
        results.ItemClick += async (_, e) =>
        {
            if (e.ClickedItem is not Border { Tag: GiphyResult r }) return;
            note.Text = "Downloading and checking…";
            string? problem = await app.Backgrounds.UseUrl(r.Full);
            if (problem is null) { dialog.Hide(); Show(null); }
            else note.Text = problem;
        };
        dialog.RequestedTheme = ActualTheme;
        try { await dialog.ShowAsync(); } catch (Exception e) { Diagnostics.Log("giphy dialog: " + e.Message); }
    }
}
