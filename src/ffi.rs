use libc;
use std::ffi::CString;
use std::ptr;
use std::slice;
use std::os::raw::{c_char, c_ulong};
use vec_map::VecMap;
use std::cell::RefCell;
use std::panic::AssertUnwindSafe;

use crate::PluginDescriptor;
use crate::get_ladspa_descriptor;

// Prevent ladspa_descriptor from being stripped during release builds
#[used] // simpler modern alternative to the old workaround function
static EXPORT_KEEPER: unsafe extern "C" fn(libc::c_ulong) -> *mut ladspa_h::Descriptor = ladspa_descriptor;

macro_rules! call_user_code {
    ($code:expr, $context:expr) => {{
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            $code
        }));
        match result {
            Ok(v) => v,
            Err(e) => {
                eprintln!("LADSPA Plugin Error in {}: {:?}", $context, e);
                Default::default()
            }
        }
    }}
}

static mut DESCRIPTORS: *mut Vec<*mut ladspa_h::Descriptor> = ptr::null_mut();

extern "C" fn global_destruct() {
    unsafe {
        if !DESCRIPTORS.is_null() {
            let descriptors = Box::from_raw(DESCRIPTORS);
            for descriptor in descriptors.iter() {
                drop_descriptor(&mut **descriptor);
            }
            DESCRIPTORS = ptr::null_mut();
        }
    }
}

unsafe fn drop_descriptor(desc: &mut ladspa_h::Descriptor) {
    unsafe {
        let _ = CString::from_raw(desc.label);
        let _ = CString::from_raw(desc.name);
        let _ = CString::from_raw(desc.maker);
        let _ = CString::from_raw(desc.copyright);

        let _ = Vec::from_raw_parts(desc.port_descriptors,
                                    desc.port_count as usize,
                                    desc.port_count as usize);

        let names = Vec::from_raw_parts(desc.port_names,
                                        desc.port_count as usize,
                                        desc.port_count as usize);
        for &ptr in names.iter() {
            let _ = CString::from_raw(ptr);
        }

        let _ = Vec::from_raw_parts(desc.port_range_hints,
                                    desc.port_count as usize,
                                    desc.port_count as usize);

        let _ = Box::from_raw(desc.implementation_data as *mut PluginDescriptor);

        let _ = Box::from_raw(desc);
    }
}

pub mod ladspa_h {
    use libc::{c_void, c_char, c_int, c_ulong, c_float};

    pub type Data = c_float;
    pub type LadspaHandle = *mut c_void;
    pub type Handle = LadspaHandle;
    pub type LadspaData = Data;

    pub type Properties = c_int;
    pub type PortDescriptor = c_int;
    pub type PortRangeHintDescriptor = c_int;

    #[repr(C)]
    #[derive(Copy, Clone)]
    pub struct PortRangeHint {
        pub hint_descriptor: PortRangeHintDescriptor,
        pub lower_bound: Data,
        pub upper_bound: Data,
    }

    #[repr(C)]
    #[allow(missing_copy_implementations)]
    pub struct Descriptor {
        pub unique_id: c_ulong,
        pub label: *mut c_char,
        pub properties: Properties,
        pub name: *mut c_char,
        pub maker: *mut c_char,
        pub copyright: *mut c_char,
        pub port_count: c_ulong,
        pub port_descriptors: *mut PortDescriptor,
        pub port_names: *mut *mut c_char,
        pub port_range_hints: *mut PortRangeHint,
        pub implementation_data: *mut c_void,
        pub instantiate: Option<unsafe extern "C" fn(descriptor: *const Descriptor, sample_rate: c_ulong) -> Handle>,
        pub connect_port: Option<unsafe extern "C" fn(instance: Handle, port: c_ulong, data_location: *mut Data)>,
        pub activate: Option<unsafe extern "C" fn(instance: Handle)>,
        pub run: Option<unsafe extern "C" fn(instance: Handle, sample_count: c_ulong)>,
        pub run_adding: Option<unsafe extern "C" fn(instance: Handle, sample_count: c_ulong)>,
        pub set_run_adding_gain: Option<unsafe extern "C" fn(instance: Handle, gain: Data)>,
        pub deactivate: Option<unsafe extern "C" fn(instance: Handle)>,
        pub cleanup: Option<unsafe extern "C" fn(instance: Handle)>,
    }

    pub const PROPERTY_REALTIME: Properties = 0x1;
    pub const PROPERTY_INPLACE_BROKEN: Properties = 0x2;
    pub const PROPERTY_HARD_RT_CAPABLE: Properties = 0x4;

    pub const PORT_INPUT: PortDescriptor = 0x1;
    pub const PORT_OUTPUT: PortDescriptor = 0x2;
    pub const PORT_CONTROL: PortDescriptor = 0x4;
    pub const PORT_AUDIO: PortDescriptor = 0x8;

    pub const HINT_BOUNDED_BELOW: PortRangeHintDescriptor = 0x1;
    pub const HINT_BOUNDED_ABOVE: PortRangeHintDescriptor = 0x2;
    pub const HINT_TOGGLED: PortRangeHintDescriptor = 0x4;
    pub const HINT_SAMPLE_RATE: PortRangeHintDescriptor = 0x8;
    pub const HINT_LOGARITHMIC: PortRangeHintDescriptor = 0x10;
    pub const HINT_INTEGER: PortRangeHintDescriptor = 0x20;
    pub const HINT_DEFAULT_MINIMUM: PortRangeHintDescriptor = 0x40;
    pub const HINT_DEFAULT_LOW: PortRangeHintDescriptor = 0x80;
    pub const HINT_DEFAULT_MIDDLE: PortRangeHintDescriptor = 0xC0;
    pub const HINT_DEFAULT_HIGH: PortRangeHintDescriptor = 0x100;
    pub const HINT_DEFAULT_MAXIMUM: PortRangeHintDescriptor = 0x140;
    pub const HINT_DEFAULT_0: PortRangeHintDescriptor = 0x200;
    pub const HINT_DEFAULT_1: PortRangeHintDescriptor = 0x240;
    pub const HINT_DEFAULT_100: PortRangeHintDescriptor = 0x280;
    pub const HINT_DEFAULT_440: PortRangeHintDescriptor = 0x2C0;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ladspa_descriptor(index: c_ulong) -> *mut ladspa_h::Descriptor {
    unsafe {
        if DESCRIPTORS.is_null() {
            libc::atexit(global_destruct);
            DESCRIPTORS = Box::into_raw(Box::new(Vec::<*mut ladspa_h::Descriptor>::new()));
        }

        let descriptors = &*DESCRIPTORS;

        if (index as usize) < descriptors.len() {
            return descriptors[index as usize];
        }

        let descriptor = call_user_code!(get_ladspa_descriptor(index), "get_ladspa_descriptor");

        match descriptor {
            None => ptr::null_mut(),
            Some(plugin) => {
                let desc = Box::into_raw(Box::new(ladspa_h::Descriptor {
                    unique_id: plugin.unique_id as c_ulong,
                    label: CString::new(plugin.label).unwrap().into_raw(),
                    properties: plugin.properties.bits(),
                    name: CString::new(plugin.name).unwrap().into_raw(),
                    maker: CString::new(plugin.maker).unwrap().into_raw(),
                    copyright: CString::new(plugin.copyright).unwrap().into_raw(),
                    port_count: plugin.ports.len() as c_ulong,
                    port_descriptors: Box::into_raw(
                        plugin.ports.iter().map(|port|
                            port.desc as i32
                        ).collect::<Vec<_>>().into_boxed_slice()) as *mut i32,
                    port_names: Box::into_raw(
                        plugin.ports.iter().map(|port|
                            CString::new(port.name).unwrap().into_raw()
                        ).collect::<Vec<_>>().into_boxed_slice()) as *mut *mut c_char,
                    port_range_hints: Box::into_raw(
                        plugin.ports.iter().map(|port|
                            ladspa_h::PortRangeHint {
                                hint_descriptor: port.hint.map(|x| x.bits()).unwrap_or(0) |
                                port.default.map(|x| x as i32).unwrap_or(0),
                                lower_bound: port.lower_bound.unwrap_or(0.0),
                                upper_bound: port.upper_bound.unwrap_or(0.0),
                            }
                        ).collect::<Vec<_>>().into_boxed_slice()) as *mut ladspa_h::PortRangeHint,
                    implementation_data: Box::into_raw(Box::new(plugin)) as *mut _,
                    instantiate: Some(instantiate),
                    connect_port: Some(connect_port),
                    activate: Some(activate),
                    run: Some(run),
                    run_adding: Some(run_adding),
                    set_run_adding_gain: Some(set_run_adding_gain),
                    deactivate: Some(deactivate),
                    cleanup: Some(cleanup),
                }));

                (*DESCRIPTORS).push(desc);
                desc
            }
        }
    }
}

// The handle that is given to ladspa.
struct Handle<'a> {
    descriptor: &'static super::PluginDescriptor,
    plugin: Box<dyn super::Plugin + Send + 'static>,
    port_map: VecMap<super::PortConnection<'a>>,
    ports: Vec<&'a super::PortConnection<'a>>,
    adding_gain: ladspa_h::Data,
    scratch_buffers: Vec<Vec<ladspa_h::Data>>,
    ptr_storage: Vec<*mut ladspa_h::Data>,
}

unsafe extern "C" fn set_run_adding_gain(instance: ladspa_h::Handle, gain: ladspa_h::Data) {
    unsafe {
        let handle = &mut *(instance as *mut Handle);
        handle.adding_gain = gain;
    }
}

unsafe extern "C" fn run_adding(instance: ladspa_h::Handle, sample_count: c_ulong) {
    unsafe {
        let handle = &mut *(instance as *mut Handle);
        let samples = sample_count as usize;

        // 1. Prepare Scratch Buffers
        // Ensure we have enough buffers for all output ports
        let num_outputs = handle.ports.iter()
            .filter(|p| matches!(p.data, super::PortData::AudioOutput(_)))
            .count();

        if handle.scratch_buffers.len() < num_outputs {
            handle.scratch_buffers.resize(num_outputs, Vec::new());
        }

        // Resize inner buffers to match block size
        for buf in &mut handle.scratch_buffers {
            if buf.len() < samples {
                buf.resize(samples, 0.0);
            }
        }

        // 2. Redirect Output Ports to Scratch Buffers
        handle.ptr_storage.clear(); // Re-use storage to avoid allocation
        let mut scratch_iter = handle.scratch_buffers.iter_mut();

        for (_, port) in handle.port_map.iter_mut() {
            match port.data {
                super::PortData::AudioOutput(ref mut cell) => {
                    // Save the actual host pointer
                    let mut slice_ref = cell.borrow_mut();
                    handle.ptr_storage.push(slice_ref.as_mut_ptr());

                    // Point the port data to our scratch buffer
                    let scratch = scratch_iter.next().unwrap();
                    *slice_ref = slice::from_raw_parts_mut(scratch.as_mut_ptr(), samples);
                },
                super::PortData::AudioInput(ref mut slice) => {
                    // Just update length (standard run behavior)
                    *slice = slice::from_raw_parts(slice.as_ptr(), samples);
                },
                _ => {}
            }
        }

        // 3. Run the Plugin (Writes to scratch buffers)
        call_user_code!({
            handle.plugin.run(samples, &handle.ports);
            Some(())
        }, "Plugin::run_adding");

        // 4. Mix Scratch into Host Buffers and Restore Pointers
        let mut host_ptr_iter = handle.ptr_storage.iter();
        let mut scratch_iter = handle.scratch_buffers.iter();

        for (_, port) in handle.port_map.iter_mut() {
            if let super::PortData::AudioOutput(ref mut cell) = port.data {
                let host_ptr = *host_ptr_iter.next().unwrap();
                let scratch_buf = scratch_iter.next().unwrap();

                // Mix: Host += Scratch * Gain
                let host_slice = slice::from_raw_parts_mut(host_ptr, samples);
                let gain = handle.adding_gain;

                for i in 0..samples {
                    host_slice[i] += scratch_buf[i] * gain;
                }

                // Restore the port to point back to the host buffer
                *cell.borrow_mut() = host_slice;
            }
        }
    }
}

unsafe extern "C" fn instantiate(descriptor: *const ladspa_h::Descriptor,
                          sample_rate: c_ulong)
                          -> ladspa_h::Handle {
    unsafe {
        let desc = &*descriptor;
        let rust_desc = &*(desc.implementation_data as *const PluginDescriptor);

        let rust_plugin = match call_user_code!(Some((rust_desc.new)(rust_desc, sample_rate)),
                                                "PluginDescriptor::run") {
            Some(plug) => plug,
            None => return ptr::null_mut(),
        };
        let port_map: VecMap<super::PortConnection> = VecMap::new();
        let ports: Vec<&super::PortConnection> = Vec::new();

        Box::into_raw(Box::new(Handle {
            descriptor: rust_desc,
            plugin: rust_plugin,
            port_map,
            ports,
            adding_gain: 1.0,
            scratch_buffers: Vec::new(),
            ptr_storage: Vec::new(),
        })) as *mut _
    }
}

unsafe extern "C" fn connect_port(instance: ladspa_h::Handle,
                           port_num: c_ulong,
                           data_location: *mut ladspa_h::Data) {
    unsafe {
        let handle = &mut *(instance as *mut Handle);

        let port = handle.descriptor.ports[port_num as usize];

        // Create appropriate pointers to port data. Mutable locations are wrapped in refcells.
        let data = match port.desc {
            super::PortDescriptor::AudioInput => {
                super::PortData::AudioInput(slice::from_raw_parts(data_location, 0))
            }
            super::PortDescriptor::AudioOutput => {
                super::PortData::AudioOutput(RefCell::new(slice::from_raw_parts_mut(data_location,
                                                                                    0)))
            }
            super::PortDescriptor::ControlInput => {
                super::PortData::ControlInput(&*data_location)
            }
            super::PortDescriptor::ControlOutput => {
                super::PortData::ControlOutput(RefCell::new(&mut *data_location))
            }
            super::PortDescriptor::Invalid => panic!("Invalid port descriptor!"),
        };

        let conn = super::PortConnection {
            port,
            data,
        };
        handle.port_map.insert(port_num as usize, conn);

        // Depends on the assumption that ports will be recreated whenever port_map changes
        if handle.port_map.len() == handle.descriptor.ports.len() {
            handle.ports = handle.port_map.values().collect();
        }
    }
}

unsafe extern "C" fn run(instance: ladspa_h::Handle, sample_count: c_ulong) {
    unsafe {
        let handle = &mut *(instance as *mut Handle);
        for (_, port) in handle.port_map.iter_mut() {
            match port.data {
                super::PortData::AudioOutput(ref mut data) => {
                    let ptr = data.borrow_mut().as_mut_ptr();
                    *data.borrow_mut() = slice::from_raw_parts_mut(ptr, sample_count as usize);
                }
                super::PortData::AudioInput(ref mut data) => {
                    let ptr = data.as_ptr();
                    *data = slice::from_raw_parts(ptr, sample_count as usize);
                }
                _ => {}
            }
        }
        let mut handle = AssertUnwindSafe(handle);
        call_user_code!({
                            let handle = &mut *handle;
                            handle.plugin.run(sample_count as usize, &handle.ports);
                            Some(())
                        },
                        "Plugin::run");
    }
}

unsafe extern "C" fn activate(instance: ladspa_h::Handle) {
    unsafe {
        let handle = &mut *(instance as *mut Handle);
        let mut handle = AssertUnwindSafe(handle);
        call_user_code!({
            handle.plugin.activate();
            Some(())
        }, "Plugin::activate");
    }
}

unsafe extern "C" fn deactivate(instance: ladspa_h::Handle) {
    unsafe {
        let handle = &mut *(instance as *mut Handle);
        let mut handle = AssertUnwindSafe(handle);
        call_user_code!({
            handle.plugin.deactivate();
            Some(())
        }, "Plugin::deactivate");
    }
}

unsafe extern "C" fn cleanup(instance: ladspa_h::Handle) {
    unsafe {
        let _ = Box::from_raw(instance as *mut Handle);
    }
}
