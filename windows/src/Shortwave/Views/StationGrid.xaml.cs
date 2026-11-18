using System.Collections.ObjectModel;
using Microsoft.UI.Xaml.Controls;
using Shortwave.Core;

namespace Shortwave.Views;

/// <summary>
/// A wrapping grid of station cards. New lists are applied as changes (insert, move, remove) rather than swapped
/// in whole, so the cards that stay glide to their new places, new ones ease in (staggered on first show) and the
/// rest close up smoothly.
/// </summary>
public sealed partial class StationGrid : UserControl
{
    readonly ObservableCollection<Station> items = [];
    /// <summary>Stations whose cards should ease in when they're first prepared, with their order in that batch.</summary>
    readonly Dictionary<string, int> entering = [];

    public StationGrid()
    {
        InitializeComponent();
        Repeater.ItemsSource = items;
        Repeater.ElementPrepared += (_, e) =>
        {
            if (e.Index >= 0 && e.Index < items.Count && entering.Remove(items[e.Index].Id, out int order)) Motion.Entrance(e.Element, order);
            else Motion.Reset(e.Element);
        };
        Repeater.ElementClearing += (_, e) => Motion.Glide(e.Element, false);
    }

    public IReadOnlyList<Station> Stations
    {
        get => items;
        set => Apply(value);
    }

    void Apply(IReadOnlyList<Station> next)
    {
        bool same = next.Count == items.Count && next.Select(s => s.Id).SequenceEqual(items.Select(s => s.Id));
        if (same)
        {
            // Same stations in the same order: only refresh the ones whose details changed.
            for (int i = 0; i < next.Count; i++) if (!Equals(items[i], next[i])) items[i] = next[i];
            return;
        }
        entering.Clear();
        Motion.GlideDuring(Repeater);
        var keep = next.Select(s => s.Id).ToHashSet();
        for (int i = items.Count - 1; i >= 0; i--) if (!keep.Contains(items[i].Id)) items.RemoveAt(i);
        int added = 0;
        for (int i = 0; i < next.Count; i++)
        {
            var s = next[i];
            if (i < items.Count && items[i].Id == s.Id)
            {
                if (!Equals(items[i], s)) items[i] = s;
                continue;
            }
            int j = -1;
            for (int k = i + 1; k < items.Count; k++) if (items[k].Id == s.Id) { j = k; break; }
            if (j >= 0)
            {
                items.Move(j, i);
                if (!Equals(items[i], s)) items[i] = s;
            }
            else
            {
                entering[s.Id] = added++;
                items.Insert(i, s);
            }
        }
        while (items.Count > next.Count) items.RemoveAt(items.Count - 1);
    }
}
