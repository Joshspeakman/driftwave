using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Shortwave.Core;
using Shortwave.Model;

namespace Shortwave.Views;

public sealed partial class SearchPage : UserControl
{
    private readonly AppModel app;
    private bool filling;

    public SearchPage(AppModel app)
    {
        this.app = app;
        InitializeComponent();
        app.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName is nameof(AppModel.Countries) or nameof(AppModel.Languages) or nameof(AppModel.Tags)) FillFilters();
            if (e.PropertyName is nameof(AppModel.SearchResults) or nameof(AppModel.SearchLoading) or nameof(AppModel.SearchError) or nameof(AppModel.Search)) Refresh();
        };
        Loaded += async (_, _) =>
        {
            SearchBox.Focus(FocusState.Programmatic);
            await app.LoadFacets();
        };
        SearchBox.Text = app.Search.Name;
        FillFilters();
        Refresh();
    }

    public void FocusSearch() => SearchBox.Focus(FocusState.Keyboard);

    sealed record Choice(string Title, string Value)
    {
        public override string ToString() => Title;
    }

    void FillFilters()
    {
        filling = true;
        var s = app.Search;
        Fill(OrderBox, SearchOrders.All.Select(o => new Choice(o.Title(), o.RawValue())), s.Order.RawValue(), null);
        Fill(CountryBox, app.Countries.Select(c => new Choice(c.Name, c.Iso3166_1 ?? "")), s.CountryCode, "Any Country");
        Fill(LanguageBox, app.Languages.Select(l => new Choice(StationCase(l.Name), l.Name)), s.Language, "Any Language");
        Fill(TagBox, app.Tags.Select(t => new Choice(t.Name, t.Name)), s.Tag, "Any Tag");
        filling = false;
    }

    static string StationCase(string s) => System.Globalization.CultureInfo.CurrentCulture.TextInfo.ToTitleCase(s);

    static void Fill(ComboBox box, IEnumerable<Choice> choices, string selected, string? any)
    {
        var list = new List<Choice>();
        if (any != null) list.Add(new Choice(any, ""));
        list.AddRange(choices);
        if (selected.Length > 0 && !list.Any(c => c.Value == selected)) list.Add(new Choice(selected, selected));
        box.ItemsSource = list;
        box.SelectedItem = list.FirstOrDefault(c => c.Value == selected) ?? list.FirstOrDefault();
    }

    static string Value(ComboBox box) => (box.SelectedItem as Choice)?.Value ?? "";

    void SearchBox_TextChanged(AutoSuggestBox sender, AutoSuggestBoxTextChangedEventArgs args)
    {
        if (args.Reason != AutoSuggestionBoxTextChangeReason.UserInput) return;
        app.Search = app.Search with { Name = sender.Text };
    }

    void Filter_Changed(object sender, SelectionChangedEventArgs e)
    {
        if (filling) return;
        var order = SearchOrders.All.FirstOrDefault(o => o.RawValue() == Value(OrderBox));
        app.Search = app.Search with { Order = order, CountryCode = Value(CountryBox), Language = Value(LanguageBox), Tag = Value(TagBox) };
    }

    void Clear_Click(object sender, RoutedEventArgs e)
    {
        app.Search = new StationSearch();
        SearchBox.Text = "";
        FillFilters();
    }

    void Retry_Click(object sender, RoutedEventArgs e) => app.SearchChanged();

    void Refresh()
    {
        var s = app.Search;
        if (SearchBox.Text != s.Name && SearchBox.FocusState == FocusState.Unfocused) SearchBox.Text = s.Name;
        ClearButton.Visibility = s != new StationSearch() ? Visibility.Visible : Visibility.Collapsed;
        CountText.Text = app.SearchResults.Count > 0 ? $"{app.SearchResults.Count} stations" : "";

        bool showResults = false, loading = false, placeholder = true;
        RetryButton.Visibility = Visibility.Collapsed;
        if (s.IsEmpty)
        {
            PlaceholderGlyph.Glyph = "";
            PlaceholderTitle.Text = "Search Stations";
            PlaceholderText.Text = "Search by name, or pick a tag, country or language.";
        }
        else if (app.SearchError is { } err)
        {
            PlaceholderGlyph.Glyph = "";
            PlaceholderTitle.Text = "Unable to Retrieve Station Data";
            PlaceholderText.Text = err;
            RetryButton.Visibility = Visibility.Visible;
        }
        else if (app.SearchResults.Count == 0 && app.SearchLoading)
        {
            placeholder = false; loading = true;
        }
        else if (app.SearchResults.Count == 0)
        {
            PlaceholderGlyph.Glyph = "";
            PlaceholderTitle.Text = s.Name.Length > 0 ? $"No Results for “{s.Name}”" : "No Results";
            PlaceholderText.Text = "Check the spelling or try a new search.";
        }
        else
        {
            placeholder = false; showResults = true;
        }
        Placeholder.Visibility = placeholder ? Visibility.Visible : Visibility.Collapsed;
        LoadingState.Visibility = loading ? Visibility.Visible : Visibility.Collapsed;
        Results.Visibility = showResults ? Visibility.Visible : Visibility.Collapsed;
        TopProgress.Visibility = showResults && app.SearchLoading ? Visibility.Visible : Visibility.Collapsed;
        if (showResults) Grid.Stations = app.SearchResults;
    }
}
