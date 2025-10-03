using SuperSearch.Models;
using System.Collections.Generic;
using System.Threading;
using System.Threading.Tasks;

namespace SuperSearch.Services;

public interface ISearchService
{
    Task<IReadOnlyList<SearchResult>> SearchAsync(string query, CancellationToken cancellationToken = default);
}
