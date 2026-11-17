using Microsoft.UI.Xaml.Controls;
using Shortwave.Core;

namespace Shortwave.Views;

/// <summary>A wrapping grid of station cards.</summary>
public sealed partial class StationGrid : UserControl
{
    public StationGrid() { InitializeComponent(); }

    public IReadOnlyList<Station> Stations
    {
        get => (IReadOnlyList<Station>?)Repeater.ItemsSource ?? [];
        set => Repeater.ItemsSource = value.ToList();
    }
}
