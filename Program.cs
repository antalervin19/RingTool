using System;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using System.Windows.Forms;

namespace RingTool;

static class Program
{
    static System.Drawing.Icon LoadIcon()
    {
        try { return System.Drawing.Icon.ExtractAssociatedIcon(Environment.ProcessPath!) ?? System.Drawing.SystemIcons.Application; }
        catch { return System.Drawing.SystemIcons.Application; }
    }

    [STAThread]
    static void Main()
    {
        var app = new System.Windows.Application { ShutdownMode = System.Windows.ShutdownMode.OnExplicitShutdown };
        var ring = new RingWindow();
        var hooks = new GlobalHooks();

        var tray = new NotifyIcon
        {
            Icon = LoadIcon(),
            Text = "RingTool - drag a file & hold Alt",
            Visible = true,
            ContextMenuStrip = new ContextMenuStrip()
        };
        tray.ContextMenuStrip.Items.Add("Quit", null, (_, _) => app.Shutdown());

        void Balloon(string title, string text, ToolTipIcon icon) =>
            app.Dispatcher.BeginInvoke(() => tray.ShowBalloonTip(2500, title, text, icon));

        hooks.ShiftDuringDrag += (x, y) => app.Dispatcher.BeginInvoke(() => ring.ShowAt(x, y));
        hooks.DragEnded += () => app.Dispatcher.BeginInvoke(ring.ScheduleHide);
        hooks.EscapePressed += () => app.Dispatcher.BeginInvoke(ring.HideNow);

        ring.Dropped += (files, target) => _ = Task.Run(async () =>
        {
            var list = files.Where(File.Exists).Where(f => Converter.Accepts(f, target)).ToList();
            int skipped = files.Length - list.Count;
            if (list.Count == 0) { Balloon("Nothing to convert", $"None of the files can become {target.Label}.", ToolTipIcon.Warning); return; }

            Balloon("Converting to " + target.Label, list.Count == 1 ? Path.GetFileName(list[0]) : $"{list.Count} files", ToolTipIcon.Info);
            int ok = 0; string last = "", error = "";
            foreach (var f in list)
            {
                try { last = await Converter.RunAsync(f, target); ok++; }
                catch (Exception ex) { error = $"{Path.GetFileName(f)}: {ex.Message}"; }
            }
            if (error != "") Balloon($"{ok} done, {list.Count - ok} failed", error, ToolTipIcon.Warning);
            else Balloon("Done - " + target.Label, (ok == 1 ? Path.GetFileName(last) : $"{ok} files") + (skipped > 0 ? $" ({skipped} skipped)" : ""), ToolTipIcon.Info);
        });

        hooks.Install();
        app.Run();
        hooks.Dispose();
        tray.Dispose();
    }
}
