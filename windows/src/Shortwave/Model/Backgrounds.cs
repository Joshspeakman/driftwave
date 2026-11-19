using System.Reflection;
using System.Text.Json;
using Shortwave.Core;
using Windows.Graphics.Imaging;

namespace Shortwave.Model;

/// <summary>A GIF found on GIPHY: a small preview to show, and the full GIF to use.</summary>
public sealed record GiphyResult(string Title, string Preview, string Full);

/// <summary>
/// Your own background: a GIF (looping) or picture behind the whole window, from a file or from GIPHY. Every one is
/// checked first: a GIF that flashes more than three times a second is refused (no flashing, ever: docs/THEMES.md).
/// Files live in the data folder's Backgrounds folder; the setting names the one in use.
/// </summary>
public sealed class Backgrounds(AppPaths paths, Preferences settings)
{
    static readonly HttpClient http = new() { Timeout = TimeSpan.FromSeconds(30) };
    public string Folder => Path.Combine(paths.Support, "Backgrounds");

    /// <summary>The background in use, if its file is still there.</summary>
    public string? CurrentPath =>
        settings.BackgroundFile is { Length: > 0 } f && File.Exists(Path.Combine(Folder, f)) ? Path.Combine(Folder, f) : null;

    /// <summary>The GIPHY key: your own (Settings), else the one built into the app (DRIFTWAVE_GIPHY_KEY at build time).</summary>
    public string? GiphyKey => settings.GiphyKey is { Length: > 0 } k ? k
        : typeof(Backgrounds).Assembly.GetCustomAttributes<AssemblyMetadataAttribute>().FirstOrDefault(a => a.Key == "GiphyKey")?.Value is { Length: > 0 } b ? b : null;

    /// <summary>Uses a picture or GIF from a file: copies it in, checks it, and makes it the background. Null, or why not.</summary>
    public async Task<string?> UseFile(string source)
    {
        string ext = Path.GetExtension(source).ToLowerInvariant();
        if (ext is not (".gif" or ".png" or ".jpg" or ".jpeg" or ".webp" or ".bmp")) return "Choose a GIF, PNG, JPEG, WebP or BMP file.";
        Directory.CreateDirectory(Folder);
        string name = Guid.NewGuid().ToString("N")[..12] + ext;
        string target = Path.Combine(Folder, name);
        File.Copy(source, target);
        try
        {
            if (await Flashes(target))
            {
                File.Delete(target);
                Diagnostics.Log("background refused: flashes");
                return "This GIF flashes, so Driftwave won't use it as a background (nothing in Driftwave may flash or strobe).";
            }
        }
        catch (Exception e)
        {
            File.Delete(target);
            return "Couldn't read that picture: " + e.Message;
        }
        string? old = CurrentPath;
        settings.BackgroundFile = name;
        if (old != null) try { File.Delete(old); } catch { }
        Diagnostics.Log("background set " + name);
        return null;
    }

    public void Clear()
    {
        string? old = CurrentPath;
        settings.BackgroundFile = null;
        if (old != null) try { File.Delete(old); } catch { }
    }

    /// <summary>Downloads a GIF (from GIPHY) and uses it, with the same checks.</summary>
    public async Task<string?> UseUrl(string url)
    {
        string temp = Path.Combine(paths.Caches, "background-download.gif");
        Directory.CreateDirectory(paths.Caches);
        try
        {
            await File.WriteAllBytesAsync(temp, await http.GetByteArrayAsync(url));
            return await UseFile(temp);
        }
        catch (Exception e) { return "Couldn't download it: " + e.Message; }
        finally { try { File.Delete(temp); } catch { } }
    }

    /// <summary>GIPHY search (rated G). Throws with a readable message when it can't search.</summary>
    public async Task<List<GiphyResult>> Search(string query)
    {
        if (GiphyKey is not { } key) throw new InvalidOperationException("GIPHY search needs an API key (Settings > Appearance > Background).");
        string url = $"https://api.giphy.com/v1/gifs/search?api_key={Uri.EscapeDataString(key)}&q={Uri.EscapeDataString(query)}&limit=24&rating=g";
        using var doc = JsonDocument.Parse(await http.GetStringAsync(url));
        var list = new List<GiphyResult>();
        foreach (var g in doc.RootElement.GetProperty("data").EnumerateArray())
        {
            var images = g.GetProperty("images");
            string? Url(string kind) => images.TryGetProperty(kind, out var k) && k.TryGetProperty("url", out var u) ? u.GetString() : null;
            string? preview = Url("fixed_width") ?? Url("fixed_height"), full = Url("downsized_large") ?? Url("original");
            if (preview != null && full != null) list.Add(new GiphyResult(g.TryGetProperty("title", out var t) ? t.GetString() ?? "" : "", preview, full));
        }
        return list;
    }

    /// <summary>
    /// Whether an animation flashes: every frame is composited at a small size (GIF frames can be partial, placed
    /// over the previous ones), measured for brightness and saturated red, and the sequence is judged by FlashCheck.
    /// </summary>
    public static async Task<bool> Flashes(string path)
    {
        using var stream = await Windows.Storage.Streams.FileRandomAccessStream.OpenAsync(path, Windows.Storage.FileAccessMode.Read);
        var decoder = await BitmapDecoder.CreateAsync(stream);
        if (decoder.FrameCount < 3) return false;
        const int Size = 64;
        double scaleX = Size / (double)Math.Max(1, decoder.PixelWidth), scaleY = Size / (double)Math.Max(1, decoder.PixelHeight);
        var canvas = new byte[Size * Size * 4];
        var frames = new List<FlashCheck.Frame>();
        uint count = Math.Min(decoder.FrameCount, 400u);
        for (uint i = 0; i < count; i++)
        {
            var frame = await decoder.GetFrameAsync(i);
            double delay = 0.1;
            int left = 0, top = 0;
            try
            {
                var props = await frame.BitmapProperties.GetPropertiesAsync(["/grctlext/Delay", "/imgdesc/Left", "/imgdesc/Top"]);
                if (props.TryGetValue("/grctlext/Delay", out var d) && d.Value is ushort cs && cs > 0) delay = cs / 100.0;
                if (props.TryGetValue("/imgdesc/Left", out var l) && l.Value is ushort lv) left = lv;
                if (props.TryGetValue("/imgdesc/Top", out var tp) && tp.Value is ushort tv) top = tv;
            }
            catch { }
            uint fw = Math.Max(1, (uint)Math.Round(frame.PixelWidth * scaleX)), fh = Math.Max(1, (uint)Math.Round(frame.PixelHeight * scaleY));
            var data = await frame.GetPixelDataAsync(BitmapPixelFormat.Bgra8, BitmapAlphaMode.Straight,
                new BitmapTransform { ScaledWidth = fw, ScaledHeight = fh, InterpolationMode = BitmapInterpolationMode.Linear },
                ExifOrientationMode.IgnoreExifOrientation, ColorManagementMode.DoNotColorManage);
            var px = data.DetachPixelData();
            int ox = (int)Math.Round(left * scaleX), oy = (int)Math.Round(top * scaleY);
            for (int y = 0; y < fh && oy + y < Size; y++)
                for (int x = 0; x < fw && ox + x < Size; x++)
                {
                    int s = (int)(y * fw + x) * 4, t = ((oy + y) * Size + ox + x) * 4;
                    if (px[s + 3] < 128) continue;   // transparent: the previous frame shows through
                    canvas[t] = px[s]; canvas[t + 1] = px[s + 1]; canvas[t + 2] = px[s + 2];
                }
            double lum = 0, red = 0;
            for (int p = 0; p < canvas.Length; p += 4)
            {
                lum += FlashCheck.Luminance(canvas[p + 2], canvas[p + 1], canvas[p]);
                if (FlashCheck.IsSaturatedRed(canvas[p + 2], canvas[p + 1], canvas[p])) red++;
            }
            int n = Size * Size;
            frames.Add(new FlashCheck.Frame(lum / n, red / n, delay));
        }
        return FlashCheck.IsFlashing(frames);
    }
}
