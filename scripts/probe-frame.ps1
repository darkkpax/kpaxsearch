[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
Add-Type @'
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public class WindowResult { public string title; public int clientX; public int clientY; public bool hardClip; public int regionKind; }
public static class Probe {
    delegate bool Callback(IntPtr w,IntPtr data);
    [StructLayout(LayoutKind.Sequential)] struct Rect { public int left,top,right,bottom; }
    [StructLayout(LayoutKind.Sequential)] struct Point { public int x,y; }
    [DllImport("user32.dll")] static extern bool EnumWindows(Callback cb,IntPtr data);
    [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr w,StringBuilder text,int max);
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr w,out Rect rect);
    [DllImport("user32.dll")] static extern bool ClientToScreen(IntPtr w,ref Point point);
    [DllImport("user32.dll")] static extern int GetWindowRgn(IntPtr w,IntPtr region);
    [DllImport("gdi32.dll")] static extern IntPtr CreateRectRgn(int a,int b,int c,int d);
    [DllImport("gdi32.dll")] static extern bool PtInRegion(IntPtr r,int x,int y);
    [DllImport("gdi32.dll")] static extern bool DeleteObject(IntPtr r);
    public static List<WindowResult> Read() {
        var results=new List<WindowResult>();
        EnumWindows((w,d)=>{
            var title=new StringBuilder(256);GetWindowText(w,title,256);
            if(!title.ToString().EndsWith("kpax search"))return true;
            Rect rect;GetWindowRect(w,out rect);var origin=new Point();ClientToScreen(w,ref origin);
            var region=CreateRectRgn(0,0,0,0);int kind=GetWindowRgn(w,region);
            results.Add(new WindowResult {title=title.ToString(),clientX=origin.x-rect.left,clientY=origin.y-rect.top,hardClip=kind==3,regionKind=kind});
            DeleteObject(region);return true;
        },IntPtr.Zero);return results;
    }
}
'@
ConvertTo-Json -InputObject @([Probe]::Read()) -Compress
