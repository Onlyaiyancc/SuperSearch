using CommunityToolkit.Mvvm.ComponentModel;
using SuperSearch.Models;
using SuperSearch.Utilities;
using System.Windows.Media;

namespace SuperSearch.ViewModels;

public sealed partial class SearchResultViewModel : ObservableObject
{
    public SearchResultKind Kind { get; }
    public string Title { get; }
    public string? Subtitle { get; }
    public string? ActionHint { get; }
    public ApplicationEntry? Application { get; }
    public string? Url { get; }
    public double Score { get; }
    public ImageSource Icon { get; }

    public SearchResultViewModel(SearchResult model)
    {
        Kind = model.Kind;
        Title = model.Title;
        Subtitle = model.Subtitle;
        ActionHint = model.ActionHint;
        Application = model.Application;
        Url = model.Url;
        Score = model.Score;
        Icon = IconLoader.GetIcon(model.IconPath);
    }
}
