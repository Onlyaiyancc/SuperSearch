using System;
using System.Text.RegularExpressions;

namespace SuperSearch.Services;

public sealed partial class UrlDetector : IUrlDetector
{
    [GeneratedRegex(@"^[a-z][a-z0-9+\-.]*://", RegexOptions.IgnoreCase | RegexOptions.Compiled)]
    private static partial Regex SchemeRegex();

    [GeneratedRegex(@"^(www\.)?([a-z0-9-]+\.)+[a-z]{2,}(:[0-9]{1,5})?(/.*)?$", RegexOptions.IgnoreCase | RegexOptions.Compiled)]
    private static partial Regex DomainRegex();

    public bool TryNormalize(string input, out string normalizedUrl)
    {
        normalizedUrl = string.Empty;
        if (string.IsNullOrWhiteSpace(input))
        {
            return false;
        }

        var text = input.Trim();
        if (text.Contains(' ', StringComparison.Ordinal))
        {
            return false;
        }

        if (Uri.TryCreate(text, UriKind.Absolute, out var absolute))
        {
            normalizedUrl = absolute.ToString();
            return true;
        }

        if (SchemeRegex().IsMatch(text))
        {
            normalizedUrl = text;
            return true;
        }

        if (DomainRegex().IsMatch(text))
        {
            normalizedUrl = $"https://{text}";
            return true;
        }

        return false;
    }
}
