using Microsoft.Extensions.DependencyInjection;
using SuperSearch.Services;
using SuperSearch.Utilities;
using SuperSearch.ViewModels;
using System;
using System.Windows;
using System.Windows.Threading;

namespace SuperSearch;

public partial class App : System.Windows.Application
{
    public IServiceProvider Services { get; }

    public App()
    {
        Services = ConfigureServices();

        AppDomain.CurrentDomain.UnhandledException += (_, args) =>
        {
            if (args.ExceptionObject is Exception ex)
            {
                Log.Error("Unhandled exception", ex);
            }
            else
            {
                Log.Error("Unhandled exception: " + args.ExceptionObject);
            }
        };

        DispatcherUnhandledException += (_, args) =>
        {
            Log.Error("Dispatcher unhandled exception", args.Exception);
            args.Handled = true;
        };
    }

    protected override async void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);
        Log.Info("OnStartup invoked");

        var mainWindow = Services.GetRequiredService<MainWindow>();
        MainWindow = mainWindow;
        Log.Info("MainWindow resolved");

        if (mainWindow.DataContext is MainViewModel vm)
        {
            Log.Info("Initializing MainViewModel");
            await vm.InitializeAsync();
            Log.Info("MainViewModel initialized");
        }

        try
        {
            Log.Info("Priming window");
            var originalOpacity = mainWindow.Opacity;
            mainWindow.Opacity = 0;
            mainWindow.Show();
            mainWindow.Hide();
            mainWindow.Opacity = originalOpacity;

            await Dispatcher.InvokeAsync(mainWindow.ForceShow);
            Log.Info("Main window shown");
        }
        catch (Exception ex)
        {
            Log.Error("Failed during warm-up", ex);
            throw;
        }
    }

    protected override void OnExit(ExitEventArgs e)
    {
        Log.Info("OnExit invoked");

        if (MainWindow is MainWindow mainWindow)
        {
            mainWindow.PrepareForShutdown();
        }

        base.OnExit(e);
    }

    private static IServiceProvider ConfigureServices()
    {
        Log.Info("Configuring services");

        var services = new ServiceCollection();

        services.AddSingleton<AppUsageTracker>();
        services.AddSingleton<ILocalAppIndexer, LocalAppIndexer>();
        services.AddSingleton<IUrlDetector, UrlDetector>();
        services.AddSingleton<Utilities.FuzzyMatcher>();
        services.AddSingleton<ISearchService, SearchService>();
        services.AddSingleton<IProcessLauncher, ProcessLauncher>();
        services.AddSingleton<IBingSearchLauncher, BingSearchLauncher>();

        services.AddSingleton<MainViewModel>();
        services.AddSingleton<MainWindow>();

        return services.BuildServiceProvider();
    }
}
