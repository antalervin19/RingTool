using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Threading.Tasks;

namespace RingTool;

enum Kind { Image, Video, Audio }
sealed record Target(string Label, string Ext, string Args, string Hint = "");
sealed record Category(string Name, Target[] Targets);

static class Converter
{
    //Supported file extensions for each kind of media file
    static readonly HashSet<string> ImageExt = new() { "png", "jpg", "jpeg", "bmp", "gif", "tif", "tiff", "webp", "ico" };
    static readonly HashSet<string> VideoExt = new() { "mp4", "mkv", "mov", "avi", "webm", "wmv", "flv", "m4v", "mpg", "mpeg", "3gp", "ts" };
    static readonly HashSet<string> AudioExt = new() { "mp3", "wav", "flac", "aac", "m4a", "ogg", "opus", "wma", "aiff", "aif" };

    const string FLAT = "-vf \"split[a][b];[a]drawbox=x=0:y=0:w=iw:h=ih:color=white:t=fill[bg];[bg][b]overlay=format=auto\"";
    const string PAL = "split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse";
    const string EVEN = "-vf \"scale=trunc(iw/2)*2:trunc(ih/2)*2\"";

    // ---- image -> image
    static readonly Target[] ImageT = {
        new("PNG",  "png",  "-frames:v 1"),
        new("JPG",  "jpg",  FLAT + " -frames:v 1 -q:v 2"),
        new("WEBP", "webp", "-c:v libwebp -quality 90"),
        new("BMP",  "bmp",  "-frames:v 1"),
        new("GIF",  "gif",  "-vf \"" + PAL + "\""),
        new("TIFF", "tiff", "-frames:v 1"),
        new("ICO",  "ico",  "-frames:v 1 -vf \"scale='min(256,iw)':'min(256,ih)':force_original_aspect_ratio=decrease\"", "max 256 px"),
    };

    // ---- video -> video
    static readonly Target[] VideoT = {
        new("MP4",  "mp4",  "-c:v libx264 -crf 20 -preset medium -pix_fmt yuv420p " + EVEN + " -c:a aac -b:a 192k -movflags +faststart"),
        new("WEBM", "webm", "-c:v libvpx-vp9 -crf 32 -b:v 0 -row-mt 1 -c:a libopus -b:a 128k", "slow"),
        new("MKV",  "mkv",  "-c copy", "fast remux"),
        new("MOV",  "mov",  "-c:v libx264 -crf 20 -preset medium -pix_fmt yuv420p " + EVEN + " -c:a aac -b:a 192k"),
        new("AVI",  "avi",  "-c:v mpeg4 -q:v 3 -c:a libmp3lame -q:a 3"),
    };

    // ---- audio (from audio OR video)
    static readonly Target[] AudioT = {
        new("MP3",  "mp3",  "-vn -c:a libmp3lame -q:a 2"),
        new("WAV",  "wav",  "-vn -c:a pcm_s16le"),
        new("FLAC", "flac", "-vn -c:a flac"),
        new("M4A",  "m4a",  "-vn -c:a aac -b:a 192k"),
        new("OGG",  "ogg",  "-vn -c:a libvorbis -q:a 5"),
        new("OPUS", "opus", "-vn -c:a libopus -b:a 128k"),
    };

    // ---- video -> image
    static readonly Target[] VideoImgT = {
        new("PNG",  "png",  "-frames:v 1 -an", "first frame"),
        new("JPG",  "jpg",  "-frames:v 1 -an -q:v 2", "first frame"),
        new("WEBP", "webp", "-frames:v 1 -an -c:v libwebp -quality 90", "first frame"),
        new("GIF",  "gif",  "-t 10 -an -vf \"fps=15,scale='min(480,iw)':-1:flags=lanczos," + PAL + "\" -loop 0", "first 10 s"),
    };

    static string NormExt(string path)
    {
        string e = Path.GetExtension(path).TrimStart('.').ToLowerInvariant();
        return e switch { "jpeg" => "jpg", "tif" => "tiff", "aif" => "aiff", _ => e };
    }

    public static Kind? KindOf(string path)
    {
        string e = Path.GetExtension(path).TrimStart('.').ToLowerInvariant();
        if (ImageExt.Contains(e)) return Kind.Image;
        if (VideoExt.Contains(e)) return Kind.Video;
        if (AudioExt.Contains(e)) return Kind.Audio;
        return null;
    }

    public static Category[] CategoriesFor(string path)
    {
        var kind = KindOf(path);
        if (kind is null) return Array.Empty<Category>();
        string ext = NormExt(path);
        Category[] all = kind switch
        {
            Kind.Image => new[] { new Category("IMAGE", ImageT) },
            Kind.Video => new[] { new Category("VIDEO", VideoT), new Category("AUDIO", AudioT), new Category("IMAGE", VideoImgT) },
            _          => new[] { new Category("AUDIO", AudioT) },
        };
        return all.Select(c => new Category(c.Name, c.Targets.Where(t => t.Ext != ext).ToArray()))
                  .Where(c => c.Targets.Length > 0).ToArray();
    }

    public static bool Accepts(string path, Target t) =>
        CategoriesFor(path).Any(c => c.Targets.Contains(t));

    static string FindFfmpeg()
    {
        string a = Path.Combine(AppContext.BaseDirectory, "ffmpeg", "ffmpeg.exe");
        if (File.Exists(a)) return a;
        string b = Path.Combine(AppContext.BaseDirectory, "ffmpeg.exe");
        if (File.Exists(b)) return b;
        string c = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "RingTool", "ffmpeg", "ffmpeg.exe");
        return File.Exists(c) ? c : "ffmpeg";
    }

    public static async Task<string> RunAsync(string input, Target t)
    {
        string dir = Path.GetDirectoryName(input)!, name = Path.GetFileNameWithoutExtension(input), ext = "." + t.Ext;
        string output = Path.Combine(dir, name + ext);
        for (int i = 1; File.Exists(output); i++) output = Path.Combine(dir, $"{name} ({i}){ext}");

        var psi = new ProcessStartInfo(FindFfmpeg())
        {
            UseShellExecute = false, CreateNoWindow = true, RedirectStandardError = true,
            Arguments = $"-y -hide_banner -loglevel error -nostdin -i \"{input}\" {t.Args} \"{output}\""
        };
        Process p;
        try { p = Process.Start(psi)!; }
        catch (Win32Exception)
        { throw new FileNotFoundException("ffmpeg.exe not found. Put it in the 'ffmpeg' folder next to RingTool.exe."); }

        string err = await p.StandardError.ReadToEndAsync();
        await p.WaitForExitAsync();
        if (p.ExitCode != 0)
        {
            try { File.Delete(output); } catch { }
            string last = err.Split('\n', StringSplitOptions.RemoveEmptyEntries).LastOrDefault()?.Trim() ?? "ffmpeg failed";
            throw new InvalidOperationException(last);
        }
        return output;
    }
}
