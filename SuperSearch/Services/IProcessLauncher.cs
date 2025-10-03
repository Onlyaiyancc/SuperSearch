using System.Threading;
using System.Threading.Tasks;

namespace SuperSearch.Services;

public interface IProcessLauncher
{
    Task<bool> LaunchAsync(string target,
                           string? arguments = null,
                           string? workingDirectory = null,
                           bool runAsAdministrator = false,
                           CancellationToken cancellationToken = default);

    Task<bool> LaunchUrlAsync(string url, CancellationToken cancellationToken = default);
}
