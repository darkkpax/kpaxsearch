$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.IO;
using System.Runtime.InteropServices;
public static class ProgramIcon {
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    struct Info {
        public IntPtr icon; public int index; public uint attributes;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 260)] public string display;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 80)] public string type;
    }
    [DllImport("shell32.dll", CharSet = CharSet.Unicode)]
    static extern int SHParseDisplayName(string name, IntPtr ctx, out IntPtr pidl, uint attrs, out uint result);
    [DllImport("shell32.dll", CharSet = CharSet.Unicode)]
    static extern IntPtr SHGetFileInfo(IntPtr pidl, uint attrs, ref Info info, uint size, uint flags);
    [DllImport("user32.dll")] static extern bool DestroyIcon(IntPtr icon);
    public static string Read(string id) {
        IntPtr pidl = IntPtr.Zero; Info info = new Info();
        try {
            uint attrs;
            if (SHParseDisplayName("shell:AppsFolder\\" + id, IntPtr.Zero, out pidl, 0, out attrs) != 0) return "";
            SHGetFileInfo(pidl, 0, ref info, (uint)Marshal.SizeOf(info), 0x100 | 0x8);
            if (info.icon == IntPtr.Zero) return "";
            using (var icon = Icon.FromHandle(info.icon))
            using (var bitmap = icon.ToBitmap())
            using (var stream = new MemoryStream()) {
                bitmap.Save(stream, ImageFormat.Png);
                return "data:image/png;base64," + Convert.ToBase64String(stream.ToArray());
            }
        } catch { return ""; }
        finally {
            if (info.icon != IntPtr.Zero) DestroyIcon(info.icon);
            if (pidl != IntPtr.Zero) Marshal.FreeCoTaskMem(pidl);
        }
    }
}
'@ -ReferencedAssemblies System.Drawing
$programs = @(Get-StartApps | Sort-Object AppID -Unique | Where-Object {
    $_.Name -and $_.AppID -and
    $_.AppID -notmatch '(?i)(\.(chm|url|html?|pdf|txt|rtf|docx?|xlsx?|pptx?|md)$|^(https?|file):)' -and
    $_.Name -notmatch '(?i)^(uninstall|\u0443\u0434\u0430\u043b\u0435\u043d\u0438\u0435|\u0443\u0434\u0430\u043b\u0438\u0442\u044c)\b'
} | ForEach-Object {
    @{ id = $_.AppID; name = $_.Name; icon = [ProgramIcon]::Read($_.AppID) }
})
ConvertTo-Json -InputObject $programs -Compress -Depth 3
