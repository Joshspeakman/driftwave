using System.Runtime.InteropServices.WindowsRuntime;
using System.Security.Cryptography;
using System.Text;
using Shortwave.Core;
using Windows.Graphics.Imaging;
using Windows.Storage.Streams;

namespace Shortwave.Model;

/// <summary>Station covers: user-chosen images, then the station's favicon (cached on disk as PNG).</summary>
public sealed class ImageStore(AppPaths paths)
{
    private readonly Dictionary<string, string> memory = [];
    private readonly HashSet<string> failed = [];
    private readonly Dictionary<string, Task<string?>> inFlight = [];
    private static readonly HttpClient http = RadioBrowser.CreateClient(TimeSpan.FromSeconds(15));

    /// <summary>Raised (on the UI thread) when a station's image changes, with the cache key.</summary>
    public event Action<string>? Changed;

    public static string? Key(Station s)
    {
        if (s.CustomCover is { } c) return "cover:" + c;
        if (!string.IsNullOrEmpty(s.FaviconURL)) return "fav:" + s.FaviconURL;
        return null;
    }

    /// <summary>The cached image file for a station, if it has been loaded.</summary>
    public string? Cached(Station s) => Key(s) is { } k && memory.TryGetValue(k, out var p) ? p : null;

    public void Invalidate(Station s)
    {
        if (Key(s) is not { } k) return;
        memory.Remove(k);
        failed.Remove(k);
        Changed?.Invoke(k);
    }

    /// <summary>Loads a station's image (call on the UI thread). Returns the PNG file path, or null.</summary>
    public async Task<string?> Load(Station s)
    {
        if (Key(s) is not { } k) return null;
        if (memory.TryGetValue(k, out var m)) return m;
        if (failed.Contains(k)) return null;
        if (inFlight.TryGetValue(k, out var t)) return await t;
        var task = Task.Run(() => Fetch(s));
        inFlight[k] = task;
        string? path = await task;
        inFlight.Remove(k);
        if (path != null) { memory[k] = path; Changed?.Invoke(k); } else failed.Add(k);
        return path;
    }

    private async Task<string?> Fetch(Station s)
    {
        try
        {
            if (s.CustomCover is { } cover)
            {
                string c = Path.Combine(paths.Covers, cover);
                return File.Exists(c) ? c : null;
            }
            if (s.FaviconURL is not { } fav || !Uri.TryCreate(fav, UriKind.Absolute, out var url)) return null;
            string digest = Convert.ToHexStringLower(SHA256.HashData(Encoding.UTF8.GetBytes(fav)));
            string file = Path.Combine(paths.Favicons, digest + ".png");
            if (File.Exists(file)) return file;
            if (url.Scheme != Uri.UriSchemeHttp && url.Scheme != Uri.UriSchemeHttps) return null;
            using var resp = await http.GetAsync(url);
            if (!resp.IsSuccessStatusCode) return null;
            byte[] data = await resp.Content.ReadAsByteArrayAsync();
            byte[]? png = await PngData(data, 256);
            if (png is null) return null;
            Directory.CreateDirectory(paths.Favicons);
            await File.WriteAllBytesAsync(file, png);
            return file;
        }
        catch { return null; }
    }

    /// <summary>Decodes any image Windows can read and re-encodes it as PNG no larger than <paramref name="maxSide"/>.</summary>
    public static async Task<byte[]?> PngData(byte[] data, uint maxSide)
    {
        try
        {
            using var input = new InMemoryRandomAccessStream();
            await input.WriteAsync(data.AsBuffer());
            input.Seek(0);
            var decoder = await BitmapDecoder.CreateAsync(input);
            uint w = decoder.PixelWidth, h = decoder.PixelHeight;
            if (w < 8 || h < 8) return null;
            double scale = Math.Min(1.0, (double)maxSide / Math.Max(w, h));
            var transform = new BitmapTransform
            {
                ScaledWidth = Math.Max(1, (uint)Math.Round(w * scale)),
                ScaledHeight = Math.Max(1, (uint)Math.Round(h * scale)),
                InterpolationMode = BitmapInterpolationMode.Fant,
            };
            var pixels = await decoder.GetPixelDataAsync(BitmapPixelFormat.Bgra8, BitmapAlphaMode.Premultiplied, transform,
                ExifOrientationMode.IgnoreExifOrientation, ColorManagementMode.ColorManageToSRgb);
            using var output = new InMemoryRandomAccessStream();
            var encoder = await BitmapEncoder.CreateAsync(BitmapEncoder.PngEncoderId, output);
            encoder.SetPixelData(BitmapPixelFormat.Bgra8, BitmapAlphaMode.Premultiplied, transform.ScaledWidth, transform.ScaledHeight, 96, 96, pixels.DetachPixelData());
            await encoder.FlushAsync();
            var bytes = new byte[output.Size];
            output.Seek(0);
            await output.ReadAsync(bytes.AsBuffer(), (uint)bytes.Length, InputStreamOptions.None);
            return bytes;
        }
        catch { return null; }
    }
}
