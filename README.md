<p align="center">
  <img src="docs/logo.png" alt="RingTool logo" width="140">
</p>

<h1 align="center">RingTool</h1>

<p align="center">
  Convert files by dragging them onto a radial menu.<br>
  Drag a file, hold <b>Alt</b>, release it on the format you want. Done.
</p>

<p align="center">
  <img src="docs/demo.gif" alt="RingTool demo" width="720">
</p>

---

## What is it?

RingTool lives in your system tray and works **anywhere on Windows**: Explorer, the Desktop, anywhere you can drag a file.

1. **Drag** a file with the mouse.
2. **Hold Alt.** A ring appears around your cursor and stays put.
3. **Move** to a segment. Some segments open a second ring with more options.
4. **Release** the file on the format you want.

The converted copy is saved next to the original. **Your original file is never touched.**

<p align="center">
  <img src="docs/ring-video.gif" alt="Video file: category ring with options" width="420">
  &nbsp;
  <img src="docs/demo.gif" alt="Image file: direct options ring" width="420">
</p>

## Features

- **Smart ring:** it reads the file as you drag, so you only see options that make sense for it.
- **Two-level menu for video:** pick `VIDEO`, `AUDIO` or `IMAGE`, then the exact format.
- **Convert across types:** video to MP3, video to a still frame, video to a GIF.
- **Safe by design:** conversions are copies, and releasing in the middle or outside the ring cancels.
- **Batch:** drag several files at once and they all convert.
- **Tiny and quiet:** a tray icon and nothing else. Starts with Windows (you can turn that off in Task Manager > Startup).

## Supported conversions

| You drag...  | Category | Options |
|--------------|----------|---------|
| **Image**    | IMAGE | PNG, JPG, WEBP, BMP, GIF, TIFF, ICO |
| **Video**    | VIDEO | MP4, WEBM, MKV (fast remux), MOV, AVI |
|              | AUDIO | MP3, WAV, FLAC, M4A, OGG, OPUS |
|              | IMAGE | PNG / JPG / WEBP (first frame), GIF (first 10 seconds) |
| **Audio**    | AUDIO | MP3, WAV, FLAC, M4A, OGG, OPUS |

The file's own format is hidden, so an MP4 never offers "MP4".

<p align="center">
  <img src="docs/screenshot-ring.png" alt="RingTool ring screenshot" width="520">
</p>

## Installation

**Requirements:** Windows 10 or 11 (64-bit). No admin rights needed, and you don't need to install .NET.

1. Go to the [**Releases page**](https://github.com/antalervin19/RingTool/releases/latest).
2. Under **Assets**, download **`RingToolSetup.exe`**.
   > Don't download the `.zip` from Assets. That's the app itself, which the installer fetches for you.
3. Run `RingToolSetup.exe`. A small splash appears while it:
   - downloads the latest RingTool,
   - downloads FFmpeg (about 70 MB, the conversion engine),
   - installs everything to `%LocalAppData%\RingTool`,
   - adds RingTool to Windows startup, and starts it.
4. Look for the RingTool icon in your system tray. You're ready.

<p align="center">
  <img src="docs/installer.gif" alt="Installer splash" width="480">
</p>

> **"Windows protected your PC" warning?** The installer isn't code-signed yet, so SmartScreen may show this on first run. Click **More info**, then **Run anyway**.

### Updating

Run `RingToolSetup.exe` from the newest release again. Automatic updates are on the roadmap.

### Uninstalling

Open **Settings > Apps > Installed apps**, find **RingTool**, and choose **Uninstall**. This stops the app, removes it from startup, and deletes its folder.

## Building from source

You need the [.NET 8 SDK](https://dotnet.microsoft.com/download) on Windows.

```bash
git clone https://github.com/antalervin19/RingTool.git
cd RingTool

# FFmpeg: put ffmpeg.exe on your PATH, or run RingToolSetup once
# (RingTool also looks in %LocalAppData%\RingTool\ffmpeg)

dotnet run -c Release
```

Publish a single self-contained exe:

```bash
dotnet publish -c Release
```

Pushing a tag such as `v1.0.0` triggers the GitHub Action in `.github/workflows/release.yml`, which builds RingTool and attaches `RingTool-win-x64.zip` to a release.

## Project layout

| File | What it does |
|------|--------------|
| `Hooks.cs` | Global mouse/keyboard hooks: detects "dragging + Alt" |
| `RingWindow.cs` | The ring: drawing, hover logic, and the drop target that reads the dragged files |
| `Converter.cs` | File types, the list of formats, and the FFmpeg commands |
| `Program.cs` | Tray icon, wiring, and background conversion |

## Contributing

Contributions are welcome, whether it's a bug report, a new format, or a whole feature.

**Reporting a bug**
- Open an [issue](https://github.com/antalervin19/RingTool/issues) with your Windows version, what you dragged (file type), what you expected, and what happened.
- A short screen recording or GIF helps a lot.

**Sending code**
1. Fork the repo and create a branch: `git checkout -b feature/my-idea`
2. Make your change and test it with `dotnet run -c Release`.
3. Keep changes focused. One feature or fix per pull request.
4. Open a pull request and describe what it does and how you tested it.

**Easy first contribution: add a format.** Every conversion is one line in `Converter.cs`:

```csharp
new("MP3", "mp3", "-vn -c:a libmp3lame -q:a 2"),
//   label  ext    FFmpeg arguments (placed after -i input)
```

Add a line to the right list (`ImageT`, `VideoT`, `AudioT`, `VideoImgT`) and it shows up on the ring automatically.

**Ideas that would be great**
- A `TEXT` category for documents (Markdown, HTML, DOCX) using pandoc
- Automatic update check
- A settings window (choose the modifier key, output folder, quality presets)
- Progress bar for long video conversions
- Code signing for the installer

## Credits and licenses

- Conversions are powered by [**FFmpeg**](https://ffmpeg.org/). RingTool downloads the Windows builds from [BtbN/FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds), which are licensed under the GPL.
- RingTool itself is released under the **Creative Commons Public License**.

---

<p align="center">Made with a cursor and too many file formats.</p>
