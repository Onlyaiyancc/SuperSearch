using SuperSearch.Models;
using System.Collections.Generic;
using System.Threading;
using System.Threading.Tasks;

namespace SuperSearch.Services;

public interface ILocalAppIndexer
{
    Task<IReadOnlyList<ApplicationEntry>> GetApplicationsAsync(CancellationToken cancellationToken = default);
}
