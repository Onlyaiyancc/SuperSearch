using System;
using System.Linq;

namespace SuperSearch.Utilities;

public sealed class FuzzyMatcher
{
    public double Score(string query, string candidate)
    {
        if (string.IsNullOrWhiteSpace(query) || string.IsNullOrWhiteSpace(candidate))
        {
            return 0;
        }

        var normalizedQuery = query.Trim().ToLowerInvariant();
        var normalizedCandidate = candidate.ToLowerInvariant();

        if (normalizedCandidate.Equals(normalizedQuery, StringComparison.Ordinal))
        {
            return 1.0;
        }

        if (normalizedCandidate.StartsWith(normalizedQuery, StringComparison.Ordinal))
        {
            return 0.92;
        }

        var tokens = normalizedQuery.Split(' ', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries);
        double bestScore = ScoreToken(normalizedQuery, normalizedCandidate);

        foreach (var token in tokens)
        {
            bestScore = Math.Max(bestScore, ScoreToken(token, normalizedCandidate));
        }

        return Math.Clamp(bestScore, 0, 1);
    }

    private static double ScoreToken(string token, string candidate)
    {
        if (string.IsNullOrEmpty(token))
        {
            return 0;
        }

        if (candidate.Contains(token, StringComparison.Ordinal))
        {
            var index = candidate.IndexOf(token, StringComparison.Ordinal);
            var positionalBonus = Math.Max(0, 0.2 - index * 0.01);
            return 0.75 + positionalBonus;
        }

        return ComputeSubsequenceScore(token, candidate);
    }

    private static double ComputeSubsequenceScore(string token, string candidate)
    {
        int qIndex = 0;
        int cIndex = 0;
        int streak = 0;
        int bestStreak = 0;

        while (qIndex < token.Length && cIndex < candidate.Length)
        {
            if (token[qIndex] == candidate[cIndex])
            {
                qIndex++;
                streak++;
                bestStreak = Math.Max(bestStreak, streak);
            }
            else if (streak > 0)
            {
                streak = 0;
            }

            cIndex++;
        }

        if (qIndex < token.Length)
        {
            var coverage = (double)qIndex / token.Length;
            return 0.2 + coverage * 0.4;
        }

        var proportional = (double)token.Length / Math.Max(candidate.Length, token.Length);
        var streakBonus = Math.Min(0.3, bestStreak * 0.05);
        return 0.45 + proportional * 0.35 + streakBonus;
    }
}
