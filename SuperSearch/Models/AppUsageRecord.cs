using System;

namespace SuperSearch.Models;

public sealed class AppUsageRecord
{
    public required string Id { get; init; }
    public int LaunchCount { get; set; }
    public DateTime LastLaunchedUtc { get; set; }
}
