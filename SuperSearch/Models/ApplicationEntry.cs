using System;

namespace SuperSearch.Models;

public sealed class ApplicationEntry
{
    public required string Id { get; init; }
    public required string Name { get; init; }
    public required string LaunchPath { get; init; }
    public string? Arguments { get; init; }
    public string? WorkingDirectory { get; init; }
    public string? IconPath { get; init; }
    public string? Description { get; init; }
    public DateTime? LastLaunchedUtc { get; set; }
    public int LaunchCount { get; set; }

    public override string ToString() => Name;
}
