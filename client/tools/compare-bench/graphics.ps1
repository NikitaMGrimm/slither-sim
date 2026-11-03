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
  [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)] public struct Output {
    [MarshalAs(UnmanagedType.ByValTStr,SizeConst=32)] public string Device;
    public int Left,Top,Right,Bottom,Attached,Rotation; public IntPtr Monitor;
  }
  public class Adapter { public string name,luid; public uint vendorId,deviceId; public string[] outputs; }
  [StructLayout(LayoutKind.Sequential)] public struct Luid { public uint Low; public int High; public override string ToString(){return String.Format("luid_0x{0:x8}_0x{1:x8}",(uint)High,Low);} }
  [StructLayout(LayoutKind.Sequential)] public struct Source { public Luid Adapter; public uint Id,Mode,Status; }
  [StructLayout(LayoutKind.Sequential)] public struct Rational { public uint Numerator,Denominator; }
  [StructLayout(LayoutKind.Sequential)] public struct Target { public Luid Adapter; public uint Id,Mode,Technology,Rotation,Scaling; public Rational Refresh; public uint Scanline; public int Available; public uint Status; }
  [StructLayout(LayoutKind.Sequential)] public struct DisplayPath { public Source Source; public Target Target; public uint Flags; }
  [StructLayout(LayoutKind.Sequential)] public struct Header { public uint Type,Size; public Luid Adapter; public uint Id; }
  [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)] public struct SourceName { public Header Header; [MarshalAs(UnmanagedType.ByValTStr,SizeConst=32)]public string Name; }
  public class DisplayBinding { public string display,sourceLuid,targetLuid; public double refreshHz; }
  [DllImport("user32.dll")] static extern int GetDisplayConfigBufferSizes(uint flags,out uint paths,out uint modes);
  [DllImport("user32.dll")] static extern int QueryDisplayConfig(uint flags,ref uint paths,[Out] DisplayPath[] pathArray,ref uint modes,[Out] byte[] modeArray,IntPtr topology);
  [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int DisplayConfigGetDeviceInfo(ref SourceName name);
  public static DisplayBinding[] DisplayBindings() {
    uint paths,modes; int code=GetDisplayConfigBufferSizes(2,out paths,out modes); if(code!=0)throw new Exception("DisplayConfig sizes "+code);
    var p=new DisplayPath[paths]; var m=new byte[modes*64]; code=QueryDisplayConfig(2,ref paths,p,ref modes,m,IntPtr.Zero); if(code!=0)throw new Exception("QueryDisplayConfig "+code);
    var result=new List<DisplayBinding>();
    for(int i=0;i<paths;i++) { var n=new SourceName(); n.Header.Type=1; n.Header.Size=(uint)Marshal.SizeOf(typeof(SourceName)); n.Header.Adapter=p[i].Source.Adapter;n.Header.Id=p[i].Source.Id;
      code=DisplayConfigGetDeviceInfo(ref n); if(code!=0)throw new Exception("DisplayConfig device name "+code);
      result.Add(new DisplayBinding{display=n.Name,sourceLuid=p[i].Source.Adapter.ToString(),targetLuid=p[i].Target.Adapter.ToString(),refreshHz=p[i].Target.Refresh.Denominator==0?0:(double)p[i].Target.Refresh.Numerator/p[i].Target.Refresh.Denominator});
    }return result.ToArray();
  }
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int EnumAdapter(IntPtr self,uint index,out IntPtr adapter);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int GetDesc(IntPtr self,out Desc desc);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int GetOutputDesc(IntPtr self,out Output desc);
  [DllImport("dxgi.dll")] static extern int CreateDXGIFactory1(ref Guid iid,out IntPtr factory);
  static T Method<T>(IntPtr obj,int slot) { return (T)(object)Marshal.GetDelegateForFunctionPointer(Marshal.ReadIntPtr(Marshal.ReadIntPtr(obj),slot*IntPtr.Size),typeof(T)); }
  public static Adapter[] Adapters() {
    var id=new Guid("770aae78-f26f-4dba-a829-253c83d1b387"); IntPtr factory;
    Marshal.ThrowExceptionForHR(CreateDXGIFactory1(ref id,out factory));
    var result=new List<Adapter>();
    try { for(uint i=0;;i++) { IntPtr adapter; if(Method<EnumAdapter>(factory,12)(factory,i,out adapter)!=0)break;
      try { Desc d; Marshal.ThrowExceptionForHR(Method<GetDesc>(adapter,10)(adapter,out d));
        var outputs=new List<string>();
        for(uint j=0;;j++) { IntPtr output; if(Method<EnumAdapter>(adapter,7)(adapter,j,out output)!=0)break;
          try { Output o; Marshal.ThrowExceptionForHR(Method<GetOutputDesc>(output,7)(output,out o)); if(o.Attached!=0)outputs.Add(o.Device); }
          finally { Marshal.Release(output); }
        }
        result.Add(new Adapter{name=d.Name,luid=String.Format("luid_0x{0:x8}_0x{1:x8}",(uint)d.High,d.Low),vendorId=d.Vendor,deviceId=d.Device,outputs=outputs.ToArray()});
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
public sealed class BenchGpu : IDisposable {
  [StructLayout(LayoutKind.Sequential)] struct Value { public uint Status; public double Number; }
  public class Reading { public string instance; public double utilization; }
  [DllImport("pdh.dll",CharSet=CharSet.Unicode)] static extern uint PdhOpenQuery(string source,UIntPtr user,out IntPtr query);
  [DllImport("pdh.dll",CharSet=CharSet.Unicode,EntryPoint="PdhAddEnglishCounterW")] static extern uint Add(IntPtr query,string path,UIntPtr user,out IntPtr counter);
  [DllImport("pdh.dll")] static extern uint PdhCollectQueryData(IntPtr query);
  [DllImport("pdh.dll")] static extern uint PdhGetFormattedCounterValue(IntPtr counter,uint format,out uint type,out Value value);
  [DllImport("pdh.dll")] static extern uint PdhCloseQuery(IntPtr query);
  IntPtr query; readonly List<IntPtr> counters=new List<IntPtr>(); readonly List<string> names=new List<string>();
  static void Check(uint code) {if(code!=0)throw new Exception("GPU PDH error 0x"+code.ToString("x8"));}
  public BenchGpu(string[] instances) {
    Check(PdhOpenQuery(null,UIntPtr.Zero,out query));
    foreach(var name in instances) {IntPtr counter;Check(Add(query,"\\GPU Engine("+name+")\\Utilization Percentage",UIntPtr.Zero,out counter));counters.Add(counter);names.Add(name);}
    Check(PdhCollectQueryData(query));
  }
  public Reading[] Sample() {
    Check(PdhCollectQueryData(query));var result=new List<Reading>();
    for(int i=0;i<counters.Count;i++){uint type;Value v;Check(PdhGetFormattedCounterValue(counters[i],0x200,out type,out v));
      if(v.Status==0||v.Status==1)result.Add(new Reading{instance=names[i],utilization=v.Number});
    }return result.ToArray();
  }
  public void Dispose(){if(query!=IntPtr.Zero){PdhCloseQuery(query);query=IntPtr.Zero;}}
}
'@
