using SuperSearch.Models;
using SuperSearch.Utilities;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;

namespace SuperSearch.Services;

public sealed class SearchService : ISearchService
{
    private readonly ILocalAppIndexer _indexer;
    private readonly AppUsageTracker _usageTracker;
    private readonly FuzzyMatcher _matcher;

    public SearchService(ILocalAppIndexer indexer, AppUsageTracker usageTracker, FuzzyMatcher matcher)
    {
        _indexer = indexer;
        _usageTracker = usageTracker;
        _matcher = matcher;
    }

    public async Task<IReadOnlyList<SearchResult>> SearchAsync(string query, CancellationToken cancellationToken = default)
    {
        var apps = await _indexer.GetApplicationsAsync(cancellationToken).ConfigureAwait(false);
        var trimmed = query?.Trim() ?? string.Empty;
        var results = new List<SearchResult>();

        if (string.IsNullOrWhiteSpace(trimmed))
        {
            var top = apps
                .Select(app => new
                {
                    app,
                    usage = _usageTracker.Get(app.Id)
                })
                .OrderByDescending(x => x.usage?.LaunchCount ?? 0)
                .ThenBy(x => x.app.Name, StringComparer.OrdinalIgnoreCase)
                .Take(8)
                .Select(x => new SearchResult
                {
                    Kind = SearchResultKind.LocalApplication,
                    Title = x.app.Name,
                    Subtitle = BuildSubtitle(x.app),
                    IconPath = x.app.IconPath,
                    Score = 1.0,
                    Application = x.app,
                    ActionHint = "Enter"
                });

            results.AddRange(top);
            return results;
        }

        foreach (var app in apps)
        {
            cancellationToken.ThrowIfCancellationRequested();
            var primaryScore = _matcher.Score(trimmed, app.Name);
            var secondaryScore = string.IsNullOrWhiteSpace(app.Description)
                ? 0d
                : 0.6 * _matcher.Score(trimmed, app.Description!);

            var combinedScore = Math.Max(primaryScore, secondaryScore);
            if (combinedScore <= 0.2)
            {
                continue;
            }

            var usage = _usageTracker.Get(app.Id);
            double usageBoost = 0;
            if (usage is not null)
            {
                usageBoost += Math.Min(0.25, usage.LaunchCount * 0.05);
                var age = (DateTime.UtcNow - usage.LastLaunchedUtc).TotalDays;
                if (age <= 7)
                {
                    usageBoost += 0.2;
                }
                else if (age <= 30)
                {
                    usageBoost += 0.1;
                }
            }

            results.Add(new SearchResult
            {
                Kind = SearchResultKind.LocalApplication,
                Title = app.Name,
                Subtitle = BuildSubtitle(app),
                IconPath = app.IconPath,
                Score = combinedScore + usageBoost,
                Application = app,
                ActionHint = "Enter"
            });
        }

        return results
            .OrderByDescending(r => r.Score)
            .ThenBy(r => r.Title, StringComparer.OrdinalIgnoreCase)
            .Take(10)
            .ToList();
    }

    private static string? BuildSubtitle(ApplicationEntry app)
    {
        if (!string.IsNullOrWhiteSpace(app.Description))
        {
            return app.Description;
        }

        return app.LaunchPath;
    }
}
