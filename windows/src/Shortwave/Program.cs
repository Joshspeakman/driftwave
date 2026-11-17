using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.Windows.AppLifecycle;
using Shortwave.Integration;

namespace Shortwave;

public static class Program
{
    [STAThread]
    static int Main(string[] args)
    {
        WinRT.ComWrappersSupport.InitializeComWrappers();

        // Install helpers (scripts\install.ps1), no UI.
        if (args.Length >= 2 && args[0] == "--create-shortcut")
        {
            Native.CreateShortcut(args[1], Environment.ProcessPath!, Diagnostics.AppUserModelId, "Internet radio");
            return 0;
        }
        if (args.Length >= 1 && args[0] == "--unregister")
        {
            try { Microsoft.Windows.AppNotifications.AppNotificationManager.Default.UnregisterAll(); } catch { }
            return 0;
        }

        if (!Diagnostics.IsTestRun)
        {
            Native.SetCurrentProcessExplicitAppUserModelID(Diagnostics.AppUserModelId);
            // One instance: a second launch brings the running app to the front.
            var key = AppInstance.FindOrRegisterForKey(Diagnostics.AppName);
            if (!key.IsCurrent)
            {
                var activation = AppInstance.GetCurrent().GetActivatedEventArgs();
                Task.Run(() => key.RedirectActivationToAsync(activation).AsTask()).Wait();
                return 0;
            }
            key.Activated += (_, _) => App.Current?.DispatcherQueue.TryEnqueue(() => App.Current.ShowMainWindow());
        }

        Application.Start(p =>
        {
            var context = new DispatcherQueueSynchronizationContext(DispatcherQueue.GetForCurrentThread());
            SynchronizationContext.SetSynchronizationContext(context);
            _ = new App();
        });
        return 0;
    }
}
