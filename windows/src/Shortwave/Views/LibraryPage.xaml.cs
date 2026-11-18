using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Shortwave.Core;
using Shortwave.Model;

using Page = Shortwave.Model.Page;

namespace Shortwave.Views;

public sealed partial class LibraryPage : UserControl
{
    private readonly AppModel app;

    public LibraryPage(AppModel app)
    {
        this.app = app;
        InitializeComponent();
        app.LibraryChanged += Refresh;
        Refresh();
    }

    void Refresh()
    {
        bool empty = app.Library.Count == 0;
        EmptyState.Visibility = empty ? Visibility.Visible : Visibility.Collapsed;
        Scroller.Visibility = empty ? Visibility.Collapsed : Visibility.Visible;
        SortButton.Visibility = empty ? Visibility.Collapsed : Visibility.Visible;
        SortLabel.Text = app.Settings.LibrarySorting.Title();
        Grid.Stations = app.SortedLibrary;
    }

    void SortMenu_Opening(object sender, object e)
    {
        SortMenu.Items.Clear();
        foreach (var s in StationSortings.All)
        {
            var item = new RadioMenuFlyoutItem { Text = s.Title(), GroupName = "sort", IsChecked = app.Settings.LibrarySorting == s };
            item.Click += (_, _) => app.Settings.LibrarySorting = s;
            SortMenu.Items.Add(item);
        }
        SortMenu.Items.Add(new MenuFlyoutSeparator());
        foreach (var (title, asc) in new[] { ("Ascending", true), ("Descending", false) })
        {
            var item = new RadioMenuFlyoutItem { Text = title, GroupName = "order", IsChecked = app.Settings.LibraryAscending == asc };
            item.Click += (_, _) => app.Settings.LibraryAscending = asc;
            SortMenu.Items.Add(item);
        }
    }

    void Discover_Click(object sender, RoutedEventArgs e) => app.Page = Page.Discover;
    void AddLocal_Click(object sender, RoutedEventArgs e) => app.AddLocalStation();
    void Import_Click(object sender, RoutedEventArgs e) => app.ChooseImportFile();
    void Export_Click(object sender, RoutedEventArgs e) => app.ChooseExportFile();
    void MoreMenu_Opening(object sender, object e) => ExportItem.IsEnabled = app.Library.Count > 0;
}
