using System;
using System.Runtime.InteropServices;

namespace SuperSearch.Interop;

[Flags]
public enum HotKeyModifiers : uint
{
    MOD_NONE = 0x0000,
    MOD_ALT = 0x0001,
    MOD_CONTROL = 0x0002,
    MOD_SHIFT = 0x0004,
    MOD_WIN = 0x0008
}

public static class HotKeyInterop
{
    [DllImport("user32.dll", SetLastError = true, EntryPoint = "RegisterHotKey")]
    private static extern bool RegisterHotKeyNative(IntPtr hWnd, int id, uint fsModifiers, uint vk);

    [DllImport("user32.dll", SetLastError = true, EntryPoint = "UnregisterHotKey")]
    private static extern bool UnregisterHotKeyNative(IntPtr hWnd, int id);

    public static bool RegisterHotKey(IntPtr handle, int id, HotKeyModifiers modifiers, int virtualKey)
    {
        if (handle == IntPtr.Zero)
        {
            return false;
        }

        return RegisterHotKeyNative(handle, id, (uint)modifiers, (uint)virtualKey);
    }

    public static bool UnregisterHotKey(IntPtr handle, int id)
    {
        if (handle == IntPtr.Zero)
        {
            return false;
        }

        return UnregisterHotKeyNative(handle, id);
    }
}
