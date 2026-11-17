using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Automation.Provider;
using Microsoft.UI.Xaml.Controls;

namespace Shortwave.Views;

/// <summary>
/// The station card's root: a panel that screen readers and UI Automation see as a button (Invoke opens the
/// station's details), like the Mac card's button trait.
/// </summary>
public sealed partial class CardPanel : Grid
{
    public event Action? Invoked;

    protected override AutomationPeer OnCreateAutomationPeer() => new Peer(this);

    sealed partial class Peer(CardPanel owner) : FrameworkElementAutomationPeer(owner), IInvokeProvider
    {
        protected override AutomationControlType GetAutomationControlTypeCore() => AutomationControlType.Button;
        protected override string GetClassNameCore() => "StationCard";
        protected override object GetPatternCore(PatternInterface patternInterface) =>
            patternInterface == PatternInterface.Invoke ? this : base.GetPatternCore(patternInterface);
        public void Invoke() => owner.Invoked?.Invoke();
    }
}
