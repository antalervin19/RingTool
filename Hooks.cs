using System;
using System.Runtime.InteropServices;

namespace RingTool;

static class Native
{
    public delegate IntPtr HookProc(int nCode, IntPtr wParam, IntPtr lParam);

    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)]
    public struct MSLLHOOKSTRUCT { public POINT pt; public uint mouseData, flags, time; public IntPtr extra; }

    [DllImport("user32.dll", SetLastError = true)]
    public static extern IntPtr SetWindowsHookEx(int id, HookProc fn, IntPtr mod, uint thread);
    [DllImport("user32.dll")] public static extern bool UnhookWindowsHookEx(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr CallNextHookEx(IntPtr h, int n, IntPtr w, IntPtr l);
    [DllImport("kernel32.dll")] public static extern IntPtr GetModuleHandle(string? name);
    [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern int GetWindowLong(IntPtr h, int i);
    [DllImport("user32.dll")] public static extern int SetWindowLong(IntPtr h, int i, int v);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, int dx, int dy, uint data, UIntPtr extra);
}

sealed class GlobalHooks : IDisposable
{
    const int WH_KEYBOARD_LL = 13, WH_MOUSE_LL = 14;
    const int WM_MOUSEMOVE = 0x200, WM_LBUTTONDOWN = 0x201, WM_LBUTTONUP = 0x202;
    const int WM_KEYDOWN = 0x100, WM_SYSKEYDOWN = 0x104;

    readonly Native.HookProc _mouseProc, _keyProc;
    IntPtr _mouseHook, _keyHook;
    bool _down, _moved;
    Native.POINT _downPt;

    public event Action<int, int>? ShiftDuringDrag;
    public event Action? DragEnded;
    public event Action? EscapePressed;

    public GlobalHooks() { _mouseProc = MouseProc; _keyProc = KeyProc; }

    public void Install()
    {
        var mod = Native.GetModuleHandle(null);
        _mouseHook = Native.SetWindowsHookEx(WH_MOUSE_LL, _mouseProc, mod, 0);
        _keyHook = Native.SetWindowsHookEx(WH_KEYBOARD_LL, _keyProc, mod, 0);
    }

    IntPtr MouseProc(int n, IntPtr w, IntPtr l)
    {
        if (n >= 0)
        {
            int msg = (int)w;
            if (msg == WM_LBUTTONDOWN)
            {
                _down = true; _moved = false;
                _downPt = Marshal.PtrToStructure<Native.MSLLHOOKSTRUCT>(l).pt;
            }
            else if (msg == WM_MOUSEMOVE && _down && !_moved)
            {
                var p = Marshal.PtrToStructure<Native.MSLLHOOKSTRUCT>(l).pt;
                if (Math.Abs(p.X - _downPt.X) + Math.Abs(p.Y - _downPt.Y) > 12) _moved = true;
            }
            else if (msg == WM_LBUTTONUP)
            {
                _down = false; _moved = false;
                DragEnded?.Invoke();
            }
        }
        return Native.CallNextHookEx(_mouseHook, n, w, l);
    }

    IntPtr KeyProc(int n, IntPtr w, IntPtr l)
    {
        if (n >= 0 && ((int)w == WM_KEYDOWN || (int)w == WM_SYSKEYDOWN))
        {
            int vk = Marshal.ReadInt32(l);
            if ((vk == 0x12 || vk == 0xA4 || vk == 0xA5) && _down && _moved)
            {
                Native.GetCursorPos(out var p);
                ShiftDuringDrag?.Invoke(p.X, p.Y);
            }
            else if (vk == 0x1B) EscapePressed?.Invoke();
        }
        return Native.CallNextHookEx(_keyHook, n, w, l);
    }

    public void Dispose()
    {
        if (_mouseHook != IntPtr.Zero) Native.UnhookWindowsHookEx(_mouseHook);
        if (_keyHook != IntPtr.Zero) Native.UnhookWindowsHookEx(_keyHook);
    }
}
