"""Read-only Razer Protocol 2.5 probe via the stock Windows HID API (ctypes only).

Sends GET commands only: firmware version (00:81), serial (00:82), DPI (04:85).
usage: python -I probe.py [pid_hex]
"""
import ctypes as C
import ctypes.wintypes as W
import sys, time

PID = int(sys.argv[1], 16) if len(sys.argv) > 1 else 0x0099
setupapi, hid, k32 = C.windll.setupapi, C.windll.hid, C.windll.kernel32
k32.CreateFileW.restype = W.HANDLE
k32.CreateFileW.argtypes = [W.LPCWSTR, W.DWORD, W.DWORD, C.c_void_p, W.DWORD, W.DWORD, W.HANDLE]
k32.CloseHandle.argtypes = [W.HANDLE]
hid.HidD_SetFeature.argtypes = hid.HidD_GetFeature.argtypes = [W.HANDLE, C.c_void_p, W.ULONG]
hid.HidD_GetPreparsedData.argtypes = [W.HANDLE, C.POINTER(C.c_void_p)]
hid.HidD_FreePreparsedData.argtypes = [C.c_void_p]
hid.HidP_GetCaps.argtypes = [C.c_void_p, C.c_void_p]
setupapi.SetupDiEnumDeviceInterfaces.argtypes = [C.c_void_p, C.c_void_p, C.c_void_p, W.DWORD, C.c_void_p]
setupapi.SetupDiGetDeviceInterfaceDetailW.argtypes = [C.c_void_p, C.c_void_p, C.c_void_p, W.DWORD, C.c_void_p, C.c_void_p]

class GUID(C.Structure):
    _fields_ = [("a", W.DWORD), ("b", W.WORD), ("c", W.WORD), ("d", C.c_ubyte * 8)]

class SP_DEVICE_INTERFACE_DATA(C.Structure):
    _fields_ = [("cbSize", W.DWORD), ("guid", GUID), ("flags", W.DWORD), ("rsv", C.c_void_p)]

class HIDP_CAPS(C.Structure):
    _fields_ = [("Usage", W.USHORT), ("UsagePage", W.USHORT), ("InLen", W.USHORT),
                ("OutLen", W.USHORT), ("FeatLen", W.USHORT), ("rest", W.USHORT * 27)]

g = GUID()
hid.HidD_GetHidGuid(C.byref(g))
setupapi.SetupDiGetClassDevsW.restype = C.c_void_p
devs = setupapi.SetupDiGetClassDevsW(C.byref(g), None, None, 0x12)  # PRESENT|DEVICEINTERFACE

def paths():
    i = 0
    while True:
        d = SP_DEVICE_INTERFACE_DATA(cbSize=C.sizeof(SP_DEVICE_INTERFACE_DATA))
        if not setupapi.SetupDiEnumDeviceInterfaces(C.c_void_p(devs), None, C.byref(g), i, C.byref(d)):
            return
        need = W.DWORD()
        setupapi.SetupDiGetDeviceInterfaceDetailW(C.c_void_p(devs), C.byref(d), None, 0, C.byref(need), None)
        buf = C.create_string_buffer(need.value)
        C.cast(buf, C.POINTER(W.DWORD))[0] = 8  # cbSize of detail struct on x64
        if setupapi.SetupDiGetDeviceInterfaceDetailW(C.c_void_p(devs), C.byref(d), buf, need, None, None):
            yield C.wstring_at(C.addressof(buf) + 4)
        i += 1

def packet(size, cls, cid, txid):
    p = bytearray(91)  # report id 0 + 90 bytes
    r = memoryview(p)[1:]
    r[1], r[5], r[6], r[7] = txid, size, cls, cid
    crc = 0
    for b in r[2:88]:
        crc ^= b
    r[88] = crc
    return p

def query(h, size, cls, cid):
    for txid in (0x1F, 0x3F, 0xFF, 0x00):
        out = packet(size, cls, cid, txid)
        if not hid.HidD_SetFeature(h, (C.c_ubyte * 91).from_buffer(out), 91):
            return None, "SetFeature failed (err %d)" % k32.GetLastError()
        time.sleep(0.06)
        buf = (C.c_ubyte * 91)()
        if not hid.HidD_GetFeature(h, buf, 91):
            return None, "GetFeature failed (err %d)" % k32.GetLastError()
        r = bytes(buf)[1:]
        if r[0] == 2 and r[6] == cls and r[7] == cid:
            return r, "ok txid=0x%02X" % txid
    return r, "status=%d (no success)" % r[0]

for p in paths():
    if ("vid_1532&pid_%04x" % PID) not in p.lower():
        continue
    h = k32.CreateFileW(p, 0xC0000000, 3, None, 3, 0, None)
    if h is None or h == C.c_void_p(-1).value:
        h = k32.CreateFileW(p, 0, 3, None, 3, 0, None)  # no access: caps only
    if h is None or h == C.c_void_p(-1).value:
        print(p.split("#")[1], " (cannot open)")
        continue
    caps = HIDP_CAPS()
    pp = C.c_void_p()
    if hid.HidD_GetPreparsedData(h, C.byref(pp)):
        hid.HidP_GetCaps(pp, C.byref(caps))
        hid.HidD_FreePreparsedData(pp)
    print("%s  usage=%04X:%04X feat=%d" % (p.split("#")[1], caps.UsagePage, caps.Usage, caps.FeatLen))
    if caps.FeatLen == 91:
        r, msg = query(h, 0x16, 0x00, 0x82)
        print("   serial:", msg, r[8:30].rstrip(b"\0").decode("ascii", "replace") if r else "")
        r, msg = query(h, 0x02, 0x00, 0x81)
        print("   firmware:", msg, ("v%d.%d" % (r[8], r[9])) if r else "")
        r, msg = query(h, 0x07, 0x04, 0x85)
        if r:
            print("   dpi:", msg, "x=%d y=%d" % (r[9] << 8 | r[10], r[11] << 8 | r[12]))
        else:
            print("   dpi:", msg)
    k32.CloseHandle(h)
