namespace SuperSearch.Models;

public sealed class SearchResult
{
    public required SearchResultKind Kind { get; init; }
    public required string Title { get; init; }
    public string? Subtitle { get; init; }
    public string? IconPath { get; init; }
    public double Score { get; init; }
    public ApplicationEntry? Application { get; init; }
    public string? Url { get; init; }
    public string? ActionHint { get; init; }
}
