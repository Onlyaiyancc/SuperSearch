namespace SuperSearch.Services;

public interface IUrlDetector
{
    bool TryNormalize(string input, out string normalizedUrl);
}
