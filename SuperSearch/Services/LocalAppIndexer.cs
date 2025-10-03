using Microsoft.Win32;
using SuperSearch.Models;
using SuperSearch.Utilities;
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;

namespace SuperSearch.Services;

public sealed class LocalAppIndexer : ILocalAppIndexer
{
    private readonly SemaphoreSlim _gate = new(1, 1);
    private IReadOnlyList<ApplicationEntry>? _cache;

    public async Task<IReadOnlyList<ApplicationEntry>> GetApplicationsAsync(CancellationToken cancellationToken = default)
    {
        if (_cache is not null)
        {
            return _cache;
        }

        await _gate.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            if (_cache is null)
            {
                Log.Info("Building local application index");
                _cache = await Task.Run(() => BuildIndex(cancellationToken), cancellationToken).ConfigureAwait(false);
                Log.Info($"Local application index ready: {_cache.Count} entries");
            }
        }
        finally
        {
            _gate.Release();
        }

        return _cache;
    }

    private static IReadOnlyList<ApplicationEntry> BuildIndex(CancellationToken cancellationToken)
    {
        var entries = new Dictionary<string, ApplicationEntry>(StringComparer.OrdinalIgnoreCase);

        foreach (var path in EnumerateStartMenuShortcuts())
        {
            cancellationToken.ThrowIfCancellationRequested();
            if (!entries.ContainsKey(path))
            {
                var entry = CreateShortcutEntry(path);
                entries[path] = entry;
            }
        }

        foreach (var entry in EnumerateAppPathExecutables())
        {
            cancellationToken.ThrowIfCancellationRequested();
            entries[entry.Id] = entry;
        }

        return entries.Values
            .OrderBy(e => e.Name, StringComparer.OrdinalIgnoreCase)
            .ToList();
    }

    private static IEnumerable<string> EnumerateStartMenuShortcuts()
    {
        var startMenuPaths = new[]
        {
            Environment.GetFolderPath(Environment.SpecialFolder.StartMenu),
            Environment.GetFolderPath(Environment.SpecialFolder.CommonStartMenu)
        };

        foreach (var root in startMenuPaths)
        {
            if (string.IsNullOrWhiteSpace(root) || !Directory.Exists(root))
            {
                continue;
            }

            foreach (var pattern in new[] { "*.lnk", "*.appref-ms", "*.url" })
            {
                foreach (var file in EnumerateFilesSafe(root, pattern))
                {
                    yield return file;
                }
            }
        }
    }

    private static IEnumerable<string> EnumerateFilesSafe(string root, string pattern)
    {
        var pending = new Stack<string>();
        pending.Push(root);

        while (pending.Count > 0)
        {
            var current = pending.Pop();

            List<string> files;
            try
            {
                files = Directory.EnumerateFiles(current, pattern, SearchOption.TopDirectoryOnly).ToList();
            }
            catch (UnauthorizedAccessException ex)
            {
                Log.Info($"Access denied scanning files in '{current}': {ex.Message}");
                files = new List<string>();
            }
            catch (IOException ex)
            {
                Log.Info($"IO error scanning files in '{current}': {ex.Message}");
                files = new List<string>();
            }

            foreach (var file in files)
            {
                yield return file;
            }

            List<string> directories;
            try
            {
                directories = Directory.EnumerateDirectories(current).ToList();
            }
            catch (UnauthorizedAccessException ex)
            {
                Log.Info($"Access denied scanning directory '{current}': {ex.Message}");
                directories = new List<string>();
            }
            catch (IOException ex)
            {
                Log.Info($"IO error scanning directory '{current}': {ex.Message}");
                directories = new List<string>();
            }

            foreach (var directory in directories)
            {
                pending.Push(directory);
            }
        }
    }

    private static IEnumerable<ApplicationEntry> EnumerateAppPathExecutables()
    {
        foreach (var registryRoot in new[] { Registry.CurrentUser, Registry.LocalMachine })
        {
            using var hive = registryRoot.OpenSubKey(@"Software\\Microsoft\\Windows\\CurrentVersion\\App Paths");
            if (hive is null)
            {
                continue;
            }

            foreach (var subKeyName in hive.GetSubKeyNames())
            {
                using var appKey = hive.OpenSubKey(subKeyName);
                if (appKey is null)
                {
                    continue;
                }

                var path = appKey.GetValue(string.Empty) as string;
                if (string.IsNullOrWhiteSpace(path))
                {
                    continue;
                }

                if (!File.Exists(path))
                {
                    continue;
                }

                var name = Path.GetFileNameWithoutExtension(path);
                var description = TryGetFileDescription(path);
                yield return new ApplicationEntry
                {
                    Id = path,
                    Name = name,
                    LaunchPath = path,
                    IconPath = path,
                    Description = description,
                    WorkingDirectory = Path.GetDirectoryName(path)
                };
            }
        }
    }

    private static ApplicationEntry CreateShortcutEntry(string path)
    {
        var name = Path.GetFileNameWithoutExtension(path);
        return new ApplicationEntry
        {
            Id = path,
            Name = name,
            LaunchPath = path,
            IconPath = path,
            Description = Path.GetDirectoryName(path)
        };
    }

    private static string? TryGetFileDescription(string path)
    {
        try
        {
            var info = FileVersionInfo.GetVersionInfo(path);
            return string.IsNullOrWhiteSpace(info.FileDescription) ? info.ProductName : info.FileDescription;
        }
        catch
        {
            return null;
        }
    }
}
