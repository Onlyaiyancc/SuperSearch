using System;
using System.Collections.Concurrent;
using System.Drawing;
using System.Drawing.Imaging;
using System.IO;
using System.Windows;
using System.Windows.Media;
using System.Windows.Media.Imaging;

namespace SuperSearch.Utilities;

public static class IconLoader
{
    private static readonly ConcurrentDictionary<string, ImageSource> _cache = new(StringComparer.OrdinalIgnoreCase);
    private static readonly ImageSource _defaultIcon = BuildDefaultIcon();

    public static ImageSource GetIcon(string? path)
    {
        if (string.IsNullOrWhiteSpace(path))
        {
            return _defaultIcon;
        }

        return _cache.GetOrAdd(path, LoadInternal);
    }

    private static ImageSource LoadInternal(string path)
    {
        try
        {
            if (!File.Exists(path))
            {
                return _defaultIcon;
            }

            var icon = Icon.ExtractAssociatedIcon(path);
            if (icon is null)
            {
                return _defaultIcon;
            }

            using (icon)
            {
                using var bitmap = icon.ToBitmap();
                using var memory = new MemoryStream();
                bitmap.Save(memory, ImageFormat.Png);
                memory.Position = 0;

                var image = new BitmapImage();
                image.BeginInit();
                image.StreamSource = memory;
                image.CacheOption = BitmapCacheOption.OnLoad;
                image.EndInit();
                image.Freeze();
                return image;
            }
        }
        catch
        {
            return _defaultIcon;
        }
    }

    private static ImageSource BuildDefaultIcon()
    {
        var drawingGroup = new DrawingGroup();
        var background = new GeometryDrawing
        {
            Brush = new SolidColorBrush(System.Windows.Media.Color.FromArgb(255, 47, 128, 237)),
            Geometry = new RectangleGeometry(new Rect(0, 0, 24, 24), 4, 4)
        };
        drawingGroup.Children.Add(background);

        var lens = new GeometryDrawing
        {
            Pen = new System.Windows.Media.Pen(System.Windows.Media.Brushes.White, 2)
            {
                StartLineCap = PenLineCap.Round,
                EndLineCap = PenLineCap.Round
            },
            Geometry = Geometry.Parse("M10,6 A4,4 0 1 0 10,14 A4,4 0 1 0 10,6 M14.5,14.5 L18,18")
        };
        drawingGroup.Children.Add(lens);

        var image = new DrawingImage(drawingGroup);
        image.Freeze();
        return image;
    }
}
