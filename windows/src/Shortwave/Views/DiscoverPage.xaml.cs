using System.Globalization;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Shortwave.Core;
using Shortwave.Model;

namespace Shortwave.Views;

public sealed partial class DiscoverPage : UserControl
{
    private readonly AppModel app;

    public DiscoverPage(AppModel app)
    {
        this.app = app;
        InitializeComponent();
        app.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName is nameof(AppModel.Popular) or nameof(AppModel.Trending) or nameof(AppModel.RandomStations)
                or nameof(AppModel.DiscoverLoading) or nameof(AppModel.DiscoverError) or nameof(AppModel.Countries)) Refresh();
        };
        Loaded += async (_, _) =>
        {
            Refresh();
            var d = app.LoadDiscover();
            var f = app.LoadFacets();
            await Task.WhenAll(d, f);
        };
        Refresh();
    }

    public static string CountryName(string code)
    {
        try { return new RegionInfo(code).DisplayName; } catch { return code; }
    }

    void Refresh()
    {
        bool error = app.DiscoverError != null && app.Popular.Count == 0;
        bool loading = !error && app.Popular.Count == 0 && app.DiscoverLoading;
        ErrorState.Visibility = error ? Visibility.Visible : Visibility.Collapsed;
        ErrorText.Text = app.DiscoverError ?? "";
        LoadingState.Visibility = loading ? Visibility.Visible : Visibility.Collapsed;
        bool content = !error && !loading;
        PopularTitle.Text = $"Popular in {CountryName(app.Settings.DiscoverCountry)}";
        PopularSection.Visibility = content && app.Popular.Count > 0 ? Visibility.Visible : Visibility.Collapsed;
        TrendingSection.Visibility = content && app.Trending.Count > 0 ? Visibility.Visible : Visibility.Collapsed;
        RandomSection.Visibility = content && app.RandomStations.Count > 0 ? Visibility.Visible : Visibility.Collapsed;
        MissingNote.Visibility = content ? Visibility.Visible : Visibility.Collapsed;
        PopularGrid.Stations = app.Popular;
        TrendingGrid.Stations = app.Trending;
        RandomGrid.Stations = app.RandomStations;
        RefreshButton.IsEnabled = !app.DiscoverLoading;
        CountryButton.IsEnabled = app.Countries.Count > 0;
        CountryLabel.Text = CountryName(app.Settings.DiscoverCountry);
    }

    async void Refresh_Click(object sender, RoutedEventArgs e) => await app.LoadDiscover(force: true);
    void AddLocal_Click(object sender, RoutedEventArgs e) => app.AddLocalStation();

    void CountryFlyout_Opened(object sender, object e)
    {
        CountryFilter.Text = "";
        FillCountries("");
        CountryFilter.Focus(FocusState.Programmatic);
    }

    void CountryFilter_TextChanged(object sender, TextChangedEventArgs e) => FillCountries(CountryFilter.Text);

    void FillCountries(string filter)
    {
        CountryList.ItemsSource = app.Countries
            .Where(c => filter.Length == 0 || c.Name.Contains(filter, StringComparison.CurrentCultureIgnoreCase))
            .Select(c => new CountryItem(c)).ToList();
    }

    async void CountryList_ItemClick(object sender, ItemClickEventArgs e)
    {
        if (e.ClickedItem is not CountryItem c) return;
        CountryFlyout.Hide();
        app.Settings.DiscoverCountry = c.Facet.Iso3166_1 ?? c.Facet.Name;
        Refresh();
        await app.LoadDiscover(force: true);
    }

    sealed record CountryItem(Facet Facet)
    {
        public override string ToString() => $"{Facet.Name} ({Facet.StationCount})";
    }
}
