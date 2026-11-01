using System;
using System.Collections.Generic;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Shapes;
using System.Windows.Threading;

namespace RingTool;

sealed record Ring(double R1, double R2, double First, double Step, int Count)
{
    public int Hit(double dx, double dy)
    {
        double r = Math.Sqrt(dx * dx + dy * dy);
        if (r < R1 || r > R2) return -1;
        double ang = Math.Atan2(dx, -dy) * 180 / Math.PI;
        double rel = ((ang - (First - Step / 2)) % 360 + 360) % 360;
        int i = (int)(rel / Step);
        return i < Count ? i : -1;
    }
}

sealed class RingWindow : Window
{
    const double C = 230;
    const double R1 = 46, R2 = 108;
    const double R3 = 116, R4 = 190;

    static readonly Dictionary<string, Color> Accent = new()
    {
        ["IMAGE"] = Color.FromRgb(0x2D, 0x9C, 0xDB),
        ["VIDEO"] = Color.FromRgb(0xA8, 0x55, 0xF7),
        ["AUDIO"] = Color.FromRgb(0xF5, 0x9E, 0x0B),
    };
    static readonly Brush Dark = new SolidColorBrush(Color.FromArgb(0xD8, 0x1E, 0x22, 0x2B));
    static Brush Lit(Color c) => new SolidColorBrush(Color.FromArgb(0xF2, c.R, c.G, c.B));

    readonly Canvas _root = new() { Width = 2 * C, Height = 2 * C, AllowDrop = true };
    readonly Canvas _innerLayer = new(), _outerLayer = new();
    readonly TextBlock _center = new()
    {
        TextAlignment = TextAlignment.Center, TextWrapping = TextWrapping.Wrap, TextTrimming = TextTrimming.CharacterEllipsis,
        Foreground = Brushes.White, FontSize = 13, FontWeight = FontWeights.SemiBold, VerticalAlignment = VerticalAlignment.Center
    };
    readonly DispatcherTimer _hideTimer = new() { Interval = TimeSpan.FromMilliseconds(350) };
    readonly DispatcherTimer _nudgeTimer = new() { Interval = TimeSpan.FromMilliseconds(60) };

    Category[] _cats = Array.Empty<Category>();
    bool _direct;
    Ring? _catRing, _optRing;
    Path[] _catPaths = Array.Empty<Path>(), _optPaths = Array.Empty<Path>();
    int _cat = -1, _opt = -1;
    string _fileKey = "", _fileLabel = "";

    public event Action<string[], Target>? Dropped;

    public RingWindow()
    {
        WindowStyle = WindowStyle.None; AllowsTransparency = true; Background = Brushes.Transparent;
        Topmost = true; ShowInTaskbar = false; ShowActivated = false; ResizeMode = ResizeMode.NoResize;
        Width = Height = 2 * C;
        Content = _root;

        _root.Children.Add(new Rectangle { Width = 2 * C, Height = 2 * C, Fill = new SolidColorBrush(Color.FromArgb(2, 0, 0, 0)) });
        _root.Children.Add(_innerLayer);
        _root.Children.Add(_outerLayer);
        var box = new Grid { Width = 2 * R1 - 6, Height = 2 * R1 - 6, IsHitTestVisible = false };
        box.Children.Add(_center);
        Canvas.SetLeft(box, C - R1 + 3); Canvas.SetTop(box, C - R1 + 3);
        _root.Children.Add(box);

        _root.DragEnter += OnEnter;
        _root.DragOver += (_, e) => Over(e);
        _root.DragLeave += (_, _) => { if (_direct) SetOpt(-1); else SetCat(-1); };
        _root.Drop += OnDrop;

        _hideTimer.Tick += (_, _) => { _hideTimer.Stop(); HideNow(); };
        _nudgeTimer.Tick += (_, _) =>
        {
            _nudgeTimer.Stop();
            if (!IsVisible) return;
            Native.mouse_event(0x0001, 1, 0, 0, UIntPtr.Zero);
            Native.mouse_event(0x0001, -1, 0, 0, UIntPtr.Zero);
        };
    }

    protected override void OnSourceInitialized(EventArgs e)
    {
        base.OnSourceInitialized(e);
        var h = new WindowInteropHelper(this).Handle;
        int ex = Native.GetWindowLong(h, -20);
        Native.SetWindowLong(h, -20, ex | 0x08000000 /*NOACTIVATE*/ | 0x80 /*TOOLWINDOW*/);
    }

    static (double x, double y) Pt(double r, double deg)
    {
        double a = deg * Math.PI / 180;
        return (C + r * Math.Sin(a), C - r * Math.Cos(a));
    }

    static Geometry Sector(double r1, double r2, double a0, double a1)
    {
        var (ox0, oy0) = Pt(r2, a0); var (ox1, oy1) = Pt(r2, a1);
        var (ix1, iy1) = Pt(r1, a1); var (ix0, iy0) = Pt(r1, a0);
        var fig = new PathFigure { StartPoint = new Point(ox0, oy0), IsClosed = true };
        fig.Segments.Add(new ArcSegment(new Point(ox1, oy1), new Size(r2, r2), 0, false, SweepDirection.Clockwise, true));
        fig.Segments.Add(new LineSegment(new Point(ix1, iy1), true));
        fig.Segments.Add(new ArcSegment(new Point(ix0, iy0), new Size(r1, r1), 0, false, SweepDirection.Counterclockwise, true));
        var g = new PathGeometry(); g.Figures.Add(fig);
        return g;
    }

    static Color AccentOf(string name) => Accent.TryGetValue(name, out var c) ? c : Colors.SteelBlue;

    static Path[] DrawRing(Canvas layer, Ring ring, string[] labels, Color accent)
    {
        var paths = new Path[ring.Count];
        for (int i = 0; i < ring.Count; i++)
        {
            double mid = ring.First + i * ring.Step;
            paths[i] = new Path
            {
                Data = Sector(ring.R1, ring.R2, mid - ring.Step / 2 + 0.8, mid + ring.Step / 2 - 0.8),
                Fill = Dark, StrokeThickness = 1.5,
                Stroke = new SolidColorBrush(Color.FromArgb(0xB0, accent.R, accent.G, accent.B))
            };
            layer.Children.Add(paths[i]);
            var (lx, ly) = Pt((ring.R1 + ring.R2) / 2, mid);
            var tb = new TextBlock { Text = labels[i], Width = 70, TextAlignment = TextAlignment.Center, Foreground = Brushes.White,
                                     FontSize = 15, FontWeight = FontWeights.SemiBold, IsHitTestVisible = false };
            Canvas.SetLeft(tb, lx - 35); Canvas.SetTop(tb, ly - 10);
            layer.Children.Add(tb);
        }
        return paths;
    }

    void ResetWaiting()
    {
        _innerLayer.Children.Clear(); _outerLayer.Children.Clear();
        _cats = Array.Empty<Category>(); _catPaths = _optPaths = Array.Empty<Path>();
        _catRing = _optRing = null; _cat = _opt = -1; _direct = false; _fileKey = "";
        _center.Text = "…";
    }

    static string Short(string s) => s.Length > 16 ? s[..13] + "…" : s;

    void Configure(string[] files)
    {
        string key = string.Join("|", files);
        if (key == _fileKey) return;
        ResetWaiting();
        _fileKey = key;
        _fileLabel = files.Length == 1 ? Short(System.IO.Path.GetFileName(files[0])) : $"{files.Length} files";
        _cats = Converter.CategoriesFor(files[0]);

        if (_cats.Length == 0) { _center.Text = "Can't convert\n" + _fileLabel; return; }
        if (_cats.Length == 1) { _direct = true; _cat = 0; BuildOptions(0); }
        else
        {
            double step = 360.0 / _cats.Length;
            _catRing = new Ring(R1, R2, 0, step, _cats.Length);
            _catPaths = new Path[_cats.Length];
            for (int i = 0; i < _cats.Length; i++)
            {
                var one = DrawRing(_innerLayer, _catRing with { First = i * step, Count = 1 }, new[] { _cats[i].Name }, AccentOf(_cats[i].Name));
                _catPaths[i] = one[0];
            }
        }
        UpdateCenter();
    }

    void BuildOptions(int c)
    {
        var cat = _cats[c];
        int n = cat.Targets.Length;
        if (_direct) _optRing = new Ring(R1, R2, 0, 360.0 / n, n);
        else
        {
            double step = Math.Min(36, 330.0 / n);
            double mid = c * (360.0 / _cats.Length);
            _optRing = new Ring(R3, R4, mid - (n - 1) * step / 2, step, n);
        }
        _optPaths = DrawRing(_outerLayer, _optRing, Array.ConvertAll(cat.Targets, t => t.Label), AccentOf(cat.Name));
    }

    void SetCat(int c)
    {
        if (c == _cat) return;
        if (_cat >= 0 && _cat < _catPaths.Length) _catPaths[_cat].Fill = Dark;
        _cat = c; _opt = -1;
        _outerLayer.Children.Clear(); _optPaths = Array.Empty<Path>(); _optRing = null;
        if (c >= 0 && c < _cats.Length) { _catPaths[c].Fill = Lit(AccentOf(_cats[c].Name)); BuildOptions(c); }
        UpdateCenter();
    }

    void SetOpt(int o)
    {
        if (o == _opt) return;
        if (_opt >= 0 && _opt < _optPaths.Length) _optPaths[_opt].Fill = Dark;
        _opt = o;
        if (o >= 0 && o < _optPaths.Length) _optPaths[o].Fill = Lit(AccentOf(_cats[_cat].Name));
        UpdateCenter();
    }

    void UpdateCenter()
    {
        if (_cats.Length == 0) return;
        if (_opt >= 0)
        {
            var t = _cats[_cat].Targets[_opt];
            _center.Text = $"{_cats[_cat].Name}\n→ {t.Label}" + (t.Hint == "" ? "" : $"\n{t.Hint}");
        }
        else if (_cat >= 0 && !_direct) _center.Text = _cats[_cat].Name + "\n" + _fileLabel;
        else _center.Text = _fileLabel;
    }

    void UpdateHover(Point p)
    {
        if (_cats.Length == 0) return;
        double dx = p.X - C, dy = p.Y - C;
        if (_direct) { SetOpt(_optRing!.Hit(dx, dy)); return; }
        int c = _catRing!.Hit(dx, dy);
        if (c >= 0) SetCat(c);
        else if (Math.Sqrt(dx * dx + dy * dy) < R1) SetCat(-1);
        SetOpt(_optRing?.Hit(dx, dy) ?? -1);
    }

    void OnEnter(object s, DragEventArgs e)
    {
        if (e.Data.GetData(DataFormats.FileDrop) is string[] f && f.Length > 0) Configure(f);
        else { ResetWaiting(); _center.Text = "Not a file"; }
        Over(e);
    }

    void Over(DragEventArgs e)
    {
        UpdateHover(e.GetPosition(_root));
        e.Effects = (_cat >= 0 && _opt >= 0) ? DragDropEffects.Copy : DragDropEffects.None;
        e.Handled = true;
    }

    void OnDrop(object s, DragEventArgs e)
    {
        e.Handled = true;
        var files = e.Data.GetData(DataFormats.FileDrop) as string[];
        Target? t = (_cat >= 0 && _opt >= 0) ? _cats[_cat].Targets[_opt] : null;
        HideNow();
        if (files != null && t != null)
            Dispatcher.BeginInvoke(() => Dropped?.Invoke(files, t), DispatcherPriority.Background);
    }

    public void ShowAt(int px, int py)
    {
        _hideTimer.Stop();
        if (IsVisible) return;
        ResetWaiting();
        Place(px, py); Show(); Place(px, py);
        _nudgeTimer.Start();
    }

    void Place(int px, int py)
    {
        var dpi = VisualTreeHelper.GetDpi(this);
        double w = 2 * C * dpi.DpiScaleX;
        var b = System.Windows.Forms.Screen.FromPoint(new System.Drawing.Point(px, py)).Bounds;
        double left = Math.Max(b.Left, Math.Min(px - w / 2, b.Right - w));
        double top = Math.Max(b.Top, Math.Min(py - w / 2, b.Bottom - w));
        Left = left / dpi.DpiScaleX; Top = top / dpi.DpiScaleY;
    }

    public void ScheduleHide() { if (IsVisible) { _hideTimer.Stop(); _hideTimer.Start(); } }
    public void HideNow() { _hideTimer.Stop(); _nudgeTimer.Stop(); if (IsVisible) Hide(); ResetWaiting(); }
}
