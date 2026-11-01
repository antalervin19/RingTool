RingTool - drag a file, hold Shift, pick a conversion from the ring.

SET UP
  1. Install the .NET 8 SDK (Windows).
  2. FFmpeg: RingToolSetup downloads it for users. For your own testing, run RingToolSetup once
     (or put ffmpeg.exe on your PATH) - RingTool also looks in %LocalAppData%\RingTool\ffmpeg.
  3. dotnet run -c Release
     or: dotnet publish -c Release -r win-x64 --self-contained false

USE
  Drag a file from Explorer or the Desktop, then hold Shift. The ring appears around the cursor.
  - Image or audio file  -> one ring of target formats.
  - Video file           -> inner ring VIDEO / AUDIO / IMAGE; hover one and its options
                            fan out on an outer ring. Release on an option.
  Centre, gaps, outside the ring, or Esc = cancel. The original file is never changed;
  the converted copy is saved next to it.

ADDING FORMATS: edit the lists at the top of Converter.cs (one line per format).
NOTE: ffmpeg builds that include x264/x265 are GPL - if you ship one, ship its license/source link.
