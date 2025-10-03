using System;
using System.Diagnostics;
using System.Threading;
using System.Threading.Tasks;

namespace SuperSearch.Services;

public sealed class ProcessLauncher : IProcessLauncher
{
    public Task<bool> LaunchAsync(string target,
                                  string? arguments = null,
                                  string? workingDirectory = null,
                                  bool runAsAdministrator = false,
                                  CancellationToken cancellationToken = default)
    {
        return Task.Run(() =>
        {
            try
            {
                var startInfo = new ProcessStartInfo
                {
                    FileName = target,
                    Arguments = arguments ?? string.Empty,
                    WorkingDirectory = workingDirectory ?? string.Empty,
                    UseShellExecute = true
                };

                if (runAsAdministrator)
                {
                    startInfo.Verb = "runas";
                }

                using var process = Process.Start(startInfo);
                return process is not null;
            }
            catch (Exception ex) when (ex is InvalidOperationException or System.ComponentModel.Win32Exception)
            {
                return false;
            }
        }, cancellationToken);
    }

    public Task<bool> LaunchUrlAsync(string url, CancellationToken cancellationToken = default)
    {
        return Task.Run(() =>
        {
            try
            {
                var startInfo = new ProcessStartInfo
                {
                    FileName = url,
                    UseShellExecute = true
                };

                using var process = Process.Start(startInfo);
                return process is not null;
            }
            catch (Exception ex) when (ex is InvalidOperationException or System.ComponentModel.Win32Exception)
            {
                return false;
            }
        }, cancellationToken);
    }
}
