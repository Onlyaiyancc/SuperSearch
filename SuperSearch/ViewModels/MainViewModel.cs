using CommunityToolkit.Mvvm.ComponentModel;
using SuperSearch.Models;
using SuperSearch.Services;
using System;
using System.Collections.ObjectModel;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using System.Windows;

namespace SuperSearch.ViewModels;

public sealed partial class MainViewModel : ObservableObject
{
    private readonly ISearchService _searchService;
    private readonly IUrlDetector _urlDetector;
    private readonly IBingSearchLauncher _bingLauncher;
    private readonly IProcessLauncher _processLauncher;
    private readonly AppUsageTracker _usageTracker;
    private readonly ILocalAppIndexer _appIndexer;

    private readonly TimeSpan _searchDelay = TimeSpan.FromMilliseconds(120);
    private CancellationTokenSource? _searchCts;

    public ObservableCollection<SearchResultViewModel> Results { get; } = new();

    public bool HasResults => Results.Count > 0;

    [ObservableProperty]
    private string _query = string.Empty;

    [ObservableProperty]
    private SearchResultViewModel? _selectedResult;

    public MainViewModel(ISearchService searchService,
                         IUrlDetector urlDetector,
                         IBingSearchLauncher bingLauncher,
                         IProcessLauncher processLauncher,
                         AppUsageTracker usageTracker,
                         ILocalAppIndexer appIndexer)
    {
        _searchService = searchService;
        _urlDetector = urlDetector;
        _bingLauncher = bingLauncher;
        _processLauncher = processLauncher;
        _usageTracker = usageTracker;
        _appIndexer = appIndexer;

        Results.CollectionChanged += (_, _) => OnPropertyChanged(nameof(HasResults));
    }

    public async Task InitializeAsync()
    {
        await _usageTracker.InitializeAsync().ConfigureAwait(false);
        await _appIndexer.GetApplicationsAsync().ConfigureAwait(false);
    }

    partial void OnQueryChanged(string value)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            _searchCts?.Cancel();
            Results.Clear();
            SelectedResult = null;
            return;
        }

        ScheduleSearch(value);
    }

    public void OnWindowShown()
    {
        if (!string.IsNullOrWhiteSpace(Query))
        {
            ScheduleSearch(Query);
        }
    }

    public void OnWindowHidden()
    {
        _searchCts?.Cancel();
        SelectedResult = Results.FirstOrDefault();
        Query = string.Empty;
    }

    public async Task<bool> ForceSearchAsync()
    {
        if (string.IsNullOrWhiteSpace(Query))
        {
            return false;
        }

        await _bingLauncher.LaunchAsync(Query).ConfigureAwait(false);
        return true;
    }
    public async Task<bool> TryExecuteSelectionAsync()
    {
        var selected = SelectedResult ?? Results.FirstOrDefault();
        if (selected is null)
        {
            if (!string.IsNullOrWhiteSpace(Query))
            {
                await _bingLauncher.LaunchAsync(Query).ConfigureAwait(false);
                return true;
            }

            return false;
        }

        switch (selected.Kind)
        {
            case SearchResultKind.LocalApplication when selected.Application is not null:
            {
                var app = selected.Application;
                var launched = await _processLauncher.LaunchAsync(app.LaunchPath, app.Arguments, app.WorkingDirectory).ConfigureAwait(false);
                if (launched)
                {
                    await _usageTracker.RecordUsageAsync(app).ConfigureAwait(false);
                }
                return launched;
            }
            case SearchResultKind.Url when selected.Url is not null:
                return await _processLauncher.LaunchUrlAsync(selected.Url).ConfigureAwait(false);
            case SearchResultKind.BingFallback:
                if (!string.IsNullOrWhiteSpace(Query))
                {
                    await _bingLauncher.LaunchAsync(Query).ConfigureAwait(false);
                    return true;
                }
                break;
        }

        return false;
    }

    private void ScheduleSearch(string query)
    {
        _searchCts?.Cancel();
        var cts = new CancellationTokenSource();
        _searchCts = cts;
        var token = cts.Token;

        _ = Task.Run(async () =>
        {
            try
            {
                await Task.Delay(_searchDelay, token).ConfigureAwait(false);
                await RefreshResultsAsync(query, token).ConfigureAwait(false);
            }
            catch (OperationCanceledException)
            {
            }
        }, token);
    }

    private async Task RefreshResultsAsync(string query, CancellationToken cancellationToken)
    {
        var localResults = await _searchService.SearchAsync(query, cancellationToken).ConfigureAwait(false);
        var items = localResults.Select(r => new SearchResultViewModel(r)).ToList();

        if (_urlDetector.TryNormalize(query, out var url))
        {
            items.Insert(0, new SearchResultViewModel(new SearchResult
            {
                Kind = SearchResultKind.Url,
                Title = url,
                Subtitle = "Open URL",
                IconPath = null,
                Score = 1.1,
                Url = url,
                ActionHint = "Enter"
            }));
        }

        if (!string.IsNullOrWhiteSpace(query))
        {
            items.Add(new SearchResultViewModel(new SearchResult
            {
                Kind = SearchResultKind.BingFallback,
                Title = $"Search Bing for \"{query}\"",
                Subtitle = "Use Bing",
                IconPath = null,
                Score = 0,
                Url = null,
                ActionHint = "Enter"
            }));
        }

        await System.Windows.Application.Current.Dispatcher.InvokeAsync(() =>
        {
            Results.Clear();
            foreach (var item in items)
            {
                Results.Add(item);
            }

            SelectedResult = Results.FirstOrDefault();
        });
    }
}
