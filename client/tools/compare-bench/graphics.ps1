# Read-only DXGI identity and per-monitor mode metadata. LUIDs bind Windows
# per-process GPU-engine instances to real adapters, rather than vendor guesses.
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class BenchGraphics {
  [DllImport("user32.dll")] static extern bool SetProcessDpiAwarenessContext(IntPtr value);
  public static void PhysicalCoordinates() { SetProcessDpiAwarenessContext(new IntPtr(-4)); }
  [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] public struct Desc {
    [MarshalAs(UnmanagedType.ByValTStr,SizeConst=128)] public string Name;
    public uint Vendor,Device,Subsys,Revision;
    public UIntPtr DedicatedVideo,DedicatedSystem,Shared;
    public uint Low; public int High; public uint Flags;
  }
  public class Adapter { public string name,luid; public uint vendorId,deviceId; }
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int EnumAdapter(IntPtr self,uint index,out IntPtr adapter);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int GetDesc(IntPtr self,out Desc desc);
  [DllImport("dxgi.dll")] static extern int CreateDXGIFactory1(ref Guid iid,out IntPtr factory);
  static T Method<T>(IntPtr obj,int slot) { return (T)(object)Marshal.GetDelegateForFunctionPointer(Marshal.ReadIntPtr(Marshal.ReadIntPtr(obj),slot*IntPtr.Size),typeof(T)); }
  public static Adapter[] Adapters() {
    var id=new Guid("770aae78-f26f-4dba-a829-253c83d1b387"); IntPtr factory;
    Marshal.ThrowExceptionForHR(CreateDXGIFactory1(ref id,out factory));
    var result=new List<Adapter>();
    try { for(uint i=0;;i++) { IntPtr adapter; if(Method<EnumAdapter>(factory,12)(factory,i,out adapter)!=0)break;
      try { Desc d; Marshal.ThrowExceptionForHR(Method<GetDesc>(adapter,10)(adapter,out d));
        result.Add(new Adapter{name=d.Name,luid=String.Format("luid_0x{0:x8}_0x{1:x8}",(uint)d.High,d.Low),vendorId=d.Vendor,deviceId=d.Device});
      } finally { Marshal.Release(adapter); }
    }} finally { Marshal.Release(factory); }
    return result.ToArray();
  }
  // DEVMODEW contains a 16-byte display union; only documented scalar fields
  // after it are used. Query by the window's actual Screen.DeviceName.
  [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] public struct Mode {
    [MarshalAs(UnmanagedType.ByValTStr,SizeConst=32)] public string Device;
    public ushort Spec,Driver,Size,Extra; public uint Fields;
    public int X,Y; public uint Orientation,FixedOutput;
    public short Color,Duplex,YResolution,TTOption,Collate;
    [MarshalAs(UnmanagedType.ByValTStr,SizeConst=32)] public string Form;
    public ushort LogPixels; public uint Bits,Width,Height,Flags,Frequency;
    public uint ICMMethod,ICMIntent,Media,Dither,Reserved1,Reserved2,PanningWidth,PanningHeight;
  }
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern bool EnumDisplaySettings(string device,int mode,ref Mode data);
  public static uint Refresh(string device) { Mode m=new Mode(); m.Size=(ushort)Marshal.SizeOf(typeof(Mode)); if(!EnumDisplaySettings(device,-1,ref m))return 0; return m.Frequency; }
}
'@
