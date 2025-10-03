using System;
using System.Threading;
using System.Threading.Tasks;

namespace SuperSearch.Services;

public sealed class BingSearchLauncher : IBingSearchLauncher
{
    private readonly IProcessLauncher _processLauncher;

    public BingSearchLauncher(IProcessLauncher processLauncher)
    {
        _processLauncher = processLauncher;
    }

    public Task LaunchAsync(string query, CancellationToken cancellationToken = default)
    {
        var url = $"https://www.bing.com/search?q={Uri.EscapeDataString(query)}";
        return _processLauncher.LaunchUrlAsync(url, cancellationToken);
    }
}
