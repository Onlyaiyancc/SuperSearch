using SuperSearch.Models;
using System;
using System.Collections.Generic;
using System.IO;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;

namespace SuperSearch.Services;

public sealed class AppUsageTracker
{
    private readonly string _storagePath;
    private readonly SemaphoreSlim _gate = new(1, 1);
    private readonly Dictionary<string, AppUsageRecord> _records = new(StringComparer.OrdinalIgnoreCase);
    private static readonly JsonSerializerOptions s_jsonOptions = new()
    {
        WriteIndented = true,
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase
    };

    public AppUsageTracker()
    {
        var folder = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "SuperSearch");
        Directory.CreateDirectory(folder);
        _storagePath = Path.Combine(folder, "usage.json");
    }

    public async Task InitializeAsync(CancellationToken cancellationToken = default)
    {
        if (!File.Exists(_storagePath))
        {
            return;
        }

        await _gate.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            await using var stream = File.OpenRead(_storagePath);
            var data = await JsonSerializer.DeserializeAsync<List<AppUsageRecord>>(stream, s_jsonOptions, cancellationToken).ConfigureAwait(false);
            if (data is null)
            {
                return;
            }

            _records.Clear();
            foreach (var record in data)
            {
                _records[record.Id] = record;
            }
        }
        finally
        {
            _gate.Release();
        }
    }

    public AppUsageRecord? Get(string id)
    {
        if (_records.TryGetValue(id, out var record))
        {
            return record;
        }

        return null;
    }

    public IReadOnlyDictionary<string, AppUsageRecord> All => _records;

    public async Task RecordUsageAsync(ApplicationEntry entry, CancellationToken cancellationToken = default)
    {
        await _gate.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            if (!_records.TryGetValue(entry.Id, out var record))
            {
                record = new AppUsageRecord
                {
                    Id = entry.Id,
                    LaunchCount = 0,
                    LastLaunchedUtc = DateTime.UtcNow
                };
                _records[entry.Id] = record;
            }

            record.LaunchCount++;
            record.LastLaunchedUtc = DateTime.UtcNow;

            await PersistAsync(cancellationToken).ConfigureAwait(false);
        }
        finally
        {
            _gate.Release();
        }
    }

    private async Task PersistAsync(CancellationToken cancellationToken)
    {
        await using var stream = File.Create(_storagePath);
        await JsonSerializer.SerializeAsync(stream, _records.Values, s_jsonOptions, cancellationToken).ConfigureAwait(false);
    }
}
