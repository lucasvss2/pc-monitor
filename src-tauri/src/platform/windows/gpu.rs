//! Read-only telemetry through Windows and the already installed NVIDIA driver.
//! No driver/service installation and no shell or DLL lookup in the working directory.
use crate::system::gpu::{utilization, EngineSample, Gpu, GpuReading, Graphics};
use std::ffi::{c_char, c_void};
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1, DXGI_ERROR_NOT_FOUND};
type RawFunction = unsafe extern "system" fn() -> isize;

#[link(name = "pdh")]
unsafe extern "system" {
    fn PdhOpenQueryW(source: *const u16, data: usize, query: *mut isize) -> u32;
    fn PdhAddEnglishCounterW(query: isize, path: *const u16, data: usize, counter: *mut isize) -> u32;
    fn PdhCollectQueryData(query: isize) -> u32;
    fn PdhGetFormattedCounterArrayW(
        counter: isize,
        format: u32,
        bytes: *mut u32,
        count: *mut u32,
        items: *mut CounterItem,
    ) -> u32;
    fn PdhCloseQuery(query: isize) -> u32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn LoadLibraryExW(name: *const u16, file: isize, flags: u32) -> isize;
    fn GetProcAddress(module: isize, name: *const u8) -> Option<RawFunction>;
    fn FreeLibrary(module: isize) -> i32;
}

#[link(name = "gdi32")]
unsafe extern "system" {
    fn D3DKMTOpenAdapterFromLuid(adapter: *mut OpenAdapter) -> i32;
    fn D3DKMTQueryAdapterInfo(query: *const AdapterQuery) -> i32;
    fn D3DKMTCloseAdapter(adapter: *const CloseAdapter) -> i32;
}

#[repr(C)]
struct OpenAdapter {
    luid: NativeLuid,
    handle: u32,
}
#[repr(C)]
struct NativeLuid {
    low: u32,
    high: i32,
}
#[repr(C)]
struct CloseAdapter {
    handle: u32,
}
#[repr(C)]
struct AdapterQuery {
    handle: u32,
    kind: u32,
    data: *mut c_void,
    bytes: u32,
}
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct PciAddress {
    bus: u32,
    device: u32,
    function: u32,
}

#[repr(C)]
struct CounterValue {
    status: u32,
    double: f64,
}
#[repr(C)]
struct CounterItem {
    name: *const u16,
    value: CounterValue,
}

struct Adapter {
    id: u64,
    label: String,
    nvidia: bool,
    pci: Option<PciAddress>,
}

fn adapters() -> Result<Vec<Adapter>, ()> {
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }.map_err(|_| ())?;
    let mut adapters = Vec::new();
    for index in 0..32 {
        let adapter = match unsafe { factory.EnumAdapters1(index) } {
            Ok(adapter) => adapter,
            Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => break,
            Err(_) => return Err(()),
        };
        let description = unsafe { adapter.GetDesc1() }.map_err(|_| ())?;
        // DXGI_ADAPTER_FLAG_SOFTWARE: do not present a software rasterizer as hardware.
        if description.Flags & 2 != 0 {
            continue;
        }
        let id = ((description.AdapterLuid.HighPart as u32 as u64) << 32)
            | u64::from(description.AdapterLuid.LowPart);
        let vendor = match description.VendorId {
            0x10de => "NVIDIA",
            0x1002 => "AMD",
            0x8086 => "Intel",
            _ => "GPU",
        };
        adapters.push(Adapter {
            id,
            label: format!("GPU {} · {vendor}", adapters.len()),
            nvidia: description.VendorId == 0x10de,
            pci: pci_address(id),
        });
    }
    Ok(adapters)
}

fn pci_address(id: u64) -> Option<PciAddress> {
    let mut adapter = OpenAdapter { luid: NativeLuid { low: id as u32, high: (id >> 32) as i32 }, handle: 0 };
    if unsafe { D3DKMTOpenAdapterFromLuid(&mut adapter) } != 0 {
        return None;
    }
    let mut pci = PciAddress::default();
    let query = AdapterQuery {
        handle: adapter.handle,
        kind: 6, // KMTQAITYPE_ADAPTERADDRESS, documented by d3dkmthk.h.
        data: (&mut pci as *mut PciAddress).cast(),
        bytes: size_of::<PciAddress>() as u32,
    };
    let status = unsafe { D3DKMTQueryAdapterInfo(&query) };
    unsafe { D3DKMTCloseAdapter(&CloseAdapter { handle: adapter.handle }) };
    (status == 0).then_some(pci)
}

struct Counters {
    query: isize,
    counter: isize,
    primed: bool,
}

impl Counters {
    fn open() -> Option<Self> {
        let mut query = 0;
        if unsafe { PdhOpenQueryW(std::ptr::null(), 0, &mut query) } != 0 {
            return None;
        }
        let mut result = Self { query, counter: 0, primed: false };
        let path: Vec<_> = "\\GPU Engine(*)\\Utilization Percentage\0".encode_utf16().collect();
        if unsafe { PdhAddEnglishCounterW(query, path.as_ptr(), 0, &mut result.counter) } != 0 {
            return None;
        }
        Some(result)
    }

    fn read(&mut self) -> Result<Option<Vec<EngineSample>>, ()> {
        if unsafe { PdhCollectQueryData(self.query) } != 0 {
            self.primed = false;
            return Err(());
        }
        if !std::mem::replace(&mut self.primed, true) {
            return Ok(None);
        }
        let (mut bytes, mut count) = (0, 0);
        const FORMAT: u32 = 0x200 | 0x8000; // PDH_FMT_DOUBLE | PDH_FMT_NOCAP100.
        let status = unsafe {
            PdhGetFormattedCounterArrayW(self.counter, FORMAT, &mut bytes, &mut count, std::ptr::null_mut())
        };
        if status != 0x800007d2 || bytes == 0 || bytes > 16 * 1024 * 1024 {
            return Err(());
        }
        // The API stores pointers and doubles in this buffer; keep it 8-byte aligned.
        let mut buffer = vec![0u64; (bytes as usize).div_ceil(8)];
        let status = unsafe {
            PdhGetFormattedCounterArrayW(self.counter, FORMAT, &mut bytes, &mut count, buffer.as_mut_ptr().cast())
        };
        if status != 0 || count as usize > buffer.len() * 8 / size_of::<CounterItem>() {
            return Err(());
        }
        let items = unsafe { std::slice::from_raw_parts(buffer.as_ptr().cast::<CounterItem>(), count as usize) };
        let mut samples = Vec::new();
        let (start, end) = (buffer.as_ptr() as usize, buffer.as_ptr() as usize + buffer.len() * 8);
        for item in items {
            if item.value.status > 1 {
                continue;
            }
            let address = item.name as usize;
            if address < start || address >= end || address % 2 != 0 {
                continue;
            }
            let name = unsafe { std::slice::from_raw_parts(item.name, (end - address) / 2) };
            let Some(length) = name.iter().position(|ch| *ch == 0) else { continue };
            let name = String::from_utf16_lossy(&name[..length]);
            if let Some(sample) = EngineSample::parse(&name, item.value.double) {
                samples.push(sample);
            }
        }
        Ok(Some(samples))
    }
}

impl Drop for Counters {
    fn drop(&mut self) {
        unsafe { PdhCloseQuery(self.query) };
    }
}

/// Only System32 is searched. Never use PATH, the EXE directory or downloaded DLLs.
struct DriverLibrary(isize);
impl DriverLibrary {
    fn nvidia() -> Option<Self> {
        let name: Vec<_> = "nvml.dll\0".encode_utf16().collect();
        let module = unsafe { LoadLibraryExW(name.as_ptr(), 0, 0x800) }; // SEARCH_SYSTEM32.
        (module != 0).then_some(Self(module))
    }

    fn symbol(&self, name: &'static [u8]) -> Option<RawFunction> {
        unsafe { GetProcAddress(self.0, name.as_ptr()) }
    }
}
impl Drop for DriverLibrary {
    fn drop(&mut self) {
        unsafe { FreeLibrary(self.0) };
    }
}

type Init = unsafe extern "C" fn() -> i32;
type Count = unsafe extern "C" fn(*mut u32) -> i32;
type Handle = unsafe extern "C" fn(u32, *mut *mut c_void) -> i32;
type PciInfo = unsafe extern "C" fn(*mut c_void, *mut NvidiaPci) -> i32;
type Temperature = unsafe extern "C" fn(*mut c_void, u32, *mut u32) -> i32;
type TemperatureV = unsafe extern "C" fn(*mut c_void, *mut NvidiaTemperature) -> i32;

#[repr(C)]
struct NvidiaPci {
    legacy_bus_id: [c_char; 16],
    domain: u32,
    bus: u32,
    device: u32,
    device_id: u32,
    subsystem_id: u32,
    bus_id: [c_char; 32],
}
#[repr(C)]
struct NvidiaTemperature {
    version: u32,
    sensor: u32,
    temperature: i32,
}

struct Nvidia {
    initialized: bool,
    _library: DriverLibrary,
    shutdown: Init,
    count: Count,
    handle: Handle,
    pci: PciInfo,
    temperature: Temperature,
    temperature_v: Option<TemperatureV>,
}

impl Nvidia {
    fn open() -> Option<Self> {
        let library = DriverLibrary::nvidia()?;
        // These function signatures follow the public NVML C ABI. The owning
        // library outlives every function pointer and device handle used here.
        unsafe {
            let initialize = std::mem::transmute::<RawFunction, Init>(library.symbol(b"nvmlInit_v2\0")?);
            let mut result = Self {
                initialized: false,
                shutdown: std::mem::transmute::<RawFunction, Init>(library.symbol(b"nvmlShutdown\0")?),
                count: std::mem::transmute::<RawFunction, Count>(library.symbol(b"nvmlDeviceGetCount_v2\0")?),
                handle: std::mem::transmute::<RawFunction, Handle>(library.symbol(b"nvmlDeviceGetHandleByIndex_v2\0")?),
                pci: std::mem::transmute::<RawFunction, PciInfo>(library.symbol(b"nvmlDeviceGetPciInfo_v3\0")?),
                temperature: std::mem::transmute::<RawFunction, Temperature>(library.symbol(b"nvmlDeviceGetTemperature\0")?),
                temperature_v: library.symbol(b"nvmlDeviceGetTemperatureV\0").map(|pointer| std::mem::transmute::<RawFunction, TemperatureV>(pointer)),
                _library: library,
            };
            if initialize() != 0 {
                return None;
            }
            result.initialized = true;
            Some(result)
        }
    }

    fn read(&self, address: Option<PciAddress>) -> GpuReading {
        let Some(address) = address else { return GpuReading::Unavailable };
        let mut count = 0;
        if unsafe { (self.count)(&mut count) } != 0 || count > 32 {
            return GpuReading::Failed;
        }
        let mut matches = Vec::new();
        for index in 0..count {
            let mut handle = std::ptr::null_mut();
            let mut pci: NvidiaPci = unsafe { std::mem::zeroed() };
            if unsafe { (self.handle)(index, &mut handle) } != 0 || handle.is_null()
                || unsafe { (self.pci)(handle, &mut pci) } != 0
            {
                continue;
            }
            // A missing/ambiguous PCI association must never attach another GPU's sensor.
            let id: Vec<u8> = pci.bus_id.iter().take_while(|ch| **ch != 0).map(|ch| *ch as u8).collect();
            let id = String::from_utf8_lossy(&id);
            let function = id.rsplit_once('.').and_then(|(_, value)| u32::from_str_radix(value, 16).ok());
            if pci.domain == 0 && pci.bus == address.bus && pci.device == address.device
                && function == Some(address.function) && pci.device_id & 0xffff == 0x10de
            {
                matches.push(handle);
            }
        }
        if matches.len() != 1 {
            return GpuReading::Unavailable;
        }
        let handle = matches[0];
        if let Some(read) = self.temperature_v {
            let mut data = NvidiaTemperature { version: size_of::<NvidiaTemperature>() as u32 | (1 << 24), sensor: 0, temperature: 0 };
            let status = unsafe { read(handle, &mut data) };
            if status == 0 {
                return GpuReading::temperature(f64::from(data.temperature));
            }
            // Older drivers can expose the symbol without supporting its version.
        }
        let mut temperature = 0;
        match unsafe { (self.temperature)(handle, 0, &mut temperature) } {
            0 => GpuReading::temperature(f64::from(temperature)),
            3 => GpuReading::Unavailable, // NVML_ERROR_NOT_SUPPORTED.
            _ => GpuReading::Failed,
        }
    }
}

impl Drop for Nvidia {
    fn drop(&mut self) {
        if self.initialized {
            unsafe { (self.shutdown)() };
        }
    }
}

#[derive(Default)]
pub struct GraphicsReader {
    counters: Option<Counters>,
    nvidia: Option<Nvidia>,
}

impl GraphicsReader {
    pub fn reset(&mut self) {
        self.counters = None;
        self.nvidia = None;
    }

    pub fn read(&mut self) -> Graphics {
        let Ok(adapters) = adapters() else {
            self.reset();
            return Graphics { devices: Vec::new(), failed: true };
        };
        if self.counters.is_none() {
            self.counters = Counters::open();
        }
        let readings = self.counters.as_mut().map(Counters::read);
        if self.nvidia.is_none() && adapters.iter().any(|adapter| adapter.nvidia) {
            self.nvidia = Nvidia::open();
        }
        let devices = adapters.into_iter().map(|adapter| {
            let usage = match &readings {
                Some(Ok(Some(samples))) => utilization(adapter.id, samples),
                Some(Ok(None)) => GpuReading::Measuring,
                Some(Err(())) => GpuReading::Failed,
                None => GpuReading::Unavailable,
            };
            let temperature = if adapter.nvidia {
                self.nvidia.as_ref().map(|nvidia| nvidia.read(adapter.pci)).unwrap_or(GpuReading::Unavailable)
            } else {
                GpuReading::Unavailable
            };
            Gpu { label: adapter.label, usage, temperature }
        }).collect();
        Graphics { devices, failed: false }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_graphics_are_optional_and_keep_cpu_collection_independent() {
        let mut reader = GraphicsReader::default();
        let frame = reader.read();
        for gpu in frame.devices {
            assert!(gpu.label.starts_with("GPU "));
            if let GpuReading::Ready(value) = gpu.usage {
                assert!((0.0..=100.0).contains(&value));
            }
            if let GpuReading::Ready(value) = gpu.temperature {
                assert!((-30.0..=150.0).contains(&value));
            }
        }
        reader.reset();
    }
}
