using System.Threading;
using System.Threading.Tasks;

namespace SuperSearch.Services;

public interface IBingSearchLauncher
{
    Task LaunchAsync(string query, CancellationToken cancellationToken = default);
}
