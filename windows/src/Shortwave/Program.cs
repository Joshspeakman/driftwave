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

        // A restart (App.Restart, e.g. to finish applying a theme): wait for the old copy to go away first,
        // or the single-instance check below would hand this launch back to it.
        int restart = Array.IndexOf(args, "--restart-after");
        if (restart >= 0 && restart + 1 < args.Length && int.TryParse(args[restart + 1], out int oldPid))
        {
            try { System.Diagnostics.Process.GetProcessById(oldPid).WaitForExit(15000); } catch { }
        }

        bool quit = args.Contains("--quit");
        if (!Diagnostics.IsTestRun || quit)
        {
            Native.SetCurrentProcessExplicitAppUserModelID(Diagnostics.AppUserModelId);
            // One instance: a second launch brings the running app to the front ("--quit" asks it to quit).
            var key = AppInstance.FindOrRegisterForKey(Diagnostics.AppName);
            if (!key.IsCurrent)
            {
                var activation = AppInstance.GetCurrent().GetActivatedEventArgs();
                Task.Run(() => key.RedirectActivationToAsync(activation).AsTask()).Wait();
                return 0;
            }
            if (quit) return 0;   // nothing running
            key.Activated += (_, e) =>
            {
                string command = (e.Data as Windows.ApplicationModel.Activation.ILaunchActivatedEventArgs)?.Arguments ?? "";
                App.Current?.DispatcherQueue.TryEnqueue(() =>
                {
                    if (command.Contains("--quit")) App.Current.Quit(); else App.Current.ShowMainWindow();
                });
            };
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
