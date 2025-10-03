using System;
using System.IO;
using System.Text;

namespace SuperSearch.Utilities;

internal static class Log
{
    private static readonly string LogFolder = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "SuperSearch");
    private static readonly string LogPath = Path.Combine(LogFolder, "app.log");
    private static readonly object Gate = new();
    private static bool _initialized;

    public static void Info(string message)
        => Write("INFO", message);

    public static void Error(string message, Exception? ex = null)
        => Write("ERROR", ex is null ? message : message + "\n" + ex);

    private static void Write(string level, string message)
    {
        try
        {
            EnsureInitialized();
            var line = $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss.fff}] {level}: {message}";
            lock (Gate)
            {
                File.AppendAllText(LogPath, line + Environment.NewLine, Encoding.UTF8);
            }
        }
        catch
        {
        }
    }

    private static void EnsureInitialized()
    {
        if (_initialized)
        {
            return;
        }

        lock (Gate)
        {
            if (_initialized)
            {
                return;
            }

            Directory.CreateDirectory(LogFolder);
            _initialized = true;
        }
    }
}
