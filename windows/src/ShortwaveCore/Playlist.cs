using System.Text.RegularExpressions;

namespace Shortwave.Core;

/// <summary>Reads the stream URL out of .pls / .m3u playlist files, which many station links point to.</summary>
public static class Playlist
{
    static string PathExtension(Uri url)
    {
        string path = url.IsAbsoluteUri ? url.AbsolutePath : url.OriginalString;
        string last = path.Split('/').LastOrDefault() ?? "";
        int dot = last.LastIndexOf('.');
        return dot > 0 ? last[(dot + 1)..].ToLowerInvariant() : "";
    }

    public static bool LooksLikePlaylist(Uri url, string? contentType)
    {
        string ext = PathExtension(url);
        if (ext is "pls" or "m3u" or "xspf") return true;
        string ct = (contentType ?? "").ToLowerInvariant();
        return ct.Contains("scpls") || (ct.Contains("x-mpegurl") && ext != "m3u8") || ct.Contains("audio/mpegurl") || ct.Contains("xspf");
    }

    static readonly char[] Newlines = ['\n', '\r', '\u000B', '\u000C', '\u0085', (char)0x2028, (char)0x2029];

    /// <summary>First stream URL in the playlist text, resolved against <paramref name="baseUrl"/>.</summary>
    public static Uri? FirstStream(string text, Uri baseUrl)
    {
        var lines = text.Split(Newlines, StringSplitOptions.RemoveEmptyEntries).Select(l => l.Trim(' ', '\t')).ToList();
        foreach (var line in lines)
        {
            // PLS: File1=http://...
            if (line.StartsWith("file", StringComparison.OrdinalIgnoreCase))
            {
                int eq = line.IndexOf('=');
                if (eq >= 0 && Resolve(line[(eq + 1)..], baseUrl) is { } u) return u;
            }
        }
        // XSPF: <location>http://...</location>
        int r = text.IndexOf("<location>", StringComparison.Ordinal);
        if (r >= 0)
        {
            int s = r + "<location>".Length;
            int e = text.IndexOf("</location>", s, StringComparison.Ordinal);
            if (e >= 0 && Resolve(text[s..e], baseUrl) is { } u) return u;
        }
        // M3U: first non-comment line
        foreach (var line in lines)
        {
            if (line.Length == 0 || line.StartsWith('#') || line.StartsWith('[') || line.Contains('=')) continue;
            if (Resolve(line, baseUrl) is { } u) return u;
        }
        return null;
    }

    static Uri? Resolve(string s, Uri baseUrl)
    {
        string t = s.Trim();
        if (t.Length == 0) return null;
        // Swift's URL(string:) rejects strings with spaces and other characters that need escaping.
        if (t.Any(c => c == ' ' || c == '"' || c == '<' || c == '>' || c == '\\' || c == '^' || c == '`' || c == '{' || c == '|' || c == '}' || char.IsControl(c))) return null;
        if (!Uri.TryCreate(baseUrl, t, out var u)) return null;
        if (u.Scheme != Uri.UriSchemeHttp && u.Scheme != Uri.UriSchemeHttps) return null;
        return u;
    }
}

public static class FileNames
{
    static readonly HashSet<char> Forbidden = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

    /// <summary>Windows device names that can't be used as a file name, even with an extension.</summary>
    static readonly Regex Reserved = new(@"^(CON|PRN|AUX|NUL|COM[0-9¹²³]|LPT[0-9¹²³])$", RegexOptions.IgnoreCase | RegexOptions.CultureInvariant);

    /// <summary>Makes a string safe to use as a file name.</summary>
    public static string Sanitized(string s, int maxLength = 150)
    {
        var chars = s.Select(c => Forbidden.Contains(c) || char.IsControl(c) || c is (char)0x2028 or (char)0x2029 or '\u0085' ? '_' : c).ToArray();
        string o = TrimSpacesAndDots(new string(chars));
        if (new System.Globalization.StringInfo(o).LengthInTextElements > maxLength)
        {
            o = SubstringByTextElements(o, maxLength).Trim(' ', '\t');
        }
        // Windows also refuses a name ending in a space or dot, and device names like CON or COM1.
        o = TrimSpacesAndDots(o);
        if (o.Length == 0) return "Track";
        string stem = o.Split('.')[0].TrimEnd(' ');
        if (Reserved.IsMatch(stem)) o = "_" + o;
        return o;
    }

    static string TrimSpacesAndDots(string s) => s.Trim(' ', '\t', '.', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', '　');

    static string SubstringByTextElements(string s, int count)
    {
        var e = System.Globalization.StringInfo.GetTextElementEnumerator(s);
        int n = 0, end = 0;
        while (n < count && e.MoveNext()) { end = e.ElementIndex + ((string)e.Current).Length; n++; }
        return s[..end];
    }

    /// <summary><c>dir\name.ext</c>, or <c>dir\name (2).ext</c> etc. when that file already exists.</summary>
    public static string UniqueFilePath(string dir, string name, string ext)
    {
        string candidate = Path.Combine(dir, name + "." + ext);
        int n = 2;
        while (File.Exists(candidate) || Directory.Exists(candidate))
        {
            candidate = Path.Combine(dir, $"{name} ({n})." + ext);
            n++;
        }
        return candidate;
    }
}
