/*!
 * The ```ladspa``` crate provides an interface for writing [LADSPA](http://www.ladspa.org/)
 * plugins safely in Rust.
 */

extern crate libc;
extern crate vec_map;

use bitflags::bitflags;

#[doc(hidden)]
pub mod ffi;

use crate::ffi::ladspa_h;

#[doc(hidden)]
pub use ffi::ladspa_descriptor;

use std::cell::{RefCell, RefMut};
use std::default::Default;

#[allow(improper_ctypes)]
unsafe extern "C" {
    /**
     * Your plugin must implement this function.
     * ```get_ladspa_descriptor``` returns a description of a supported plugin for a given plugin
     * index. When the index is out of bounds for the number of plugins supported by your library,
     * you are expected to return ```None```.
     */
    pub fn get_ladspa_descriptor(index: u64) -> Option<PluginDescriptor>;
}

/// The data type used internally by LADSPA for audio and control ports.
pub type Data = f32;

/// Describes the properties of a ```Plugin``` to be exposed as a LADSPA plugin.
pub struct PluginDescriptor {
    pub unique_id: u64,
    pub label: &'static str,
    pub properties: Properties,
    pub name: &'static str,
    pub maker: &'static str,
    pub copyright: &'static str,
    pub ports: Vec<Port>,
    pub new: fn(desc: &PluginDescriptor, sample_rate: u64) -> Box<dyn Plugin + Send>,
}

#[derive(Copy, Clone, Default)]
pub struct Port {
    pub name: &'static str,
    pub desc: PortDescriptor,
    pub hint: Option<ControlHint>,
    pub default: Option<DefaultValue>,
    pub lower_bound: Option<Data>,
    pub upper_bound: Option<Data>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum PortDescriptor {
    #[default]
    Invalid = 0,
    AudioInput = (ladspa_h::PORT_AUDIO | ladspa_h::PORT_INPUT) as isize,
    AudioOutput = (ladspa_h::PORT_AUDIO | ladspa_h::PORT_OUTPUT) as isize,
    ControlInput = (ladspa_h::PORT_CONTROL | ladspa_h::PORT_INPUT) as isize,
    ControlOutput = (ladspa_h::PORT_CONTROL | ladspa_h::PORT_OUTPUT) as isize,
}

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ControlHint: i32 {
        const HINT_TOGGLED = ladspa_h::HINT_TOGGLED;
        const HINT_SAMPLE_RATE = ladspa_h::HINT_SAMPLE_RATE;
        const HINT_LOGARITHMIC = ladspa_h::HINT_LOGARITHMIC;
        const HINT_INTEGER = ladspa_h::HINT_INTEGER;
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DefaultValue {
    Minimum = ladspa_h::HINT_DEFAULT_MINIMUM as isize,
    Low = ladspa_h::HINT_DEFAULT_LOW as isize,
    Middle = ladspa_h::HINT_DEFAULT_MIDDLE as isize,
    High = ladspa_h::HINT_DEFAULT_HIGH as isize,
    Maximum = ladspa_h::HINT_DEFAULT_MAXIMUM as isize,
    Value0 = ladspa_h::HINT_DEFAULT_0 as isize,
    Value1 = ladspa_h::HINT_DEFAULT_1 as isize,
    Value100 = ladspa_h::HINT_DEFAULT_100 as isize,
    Value440 = ladspa_h::HINT_DEFAULT_440 as isize,
}

pub struct PortConnection<'a> {
    pub port: Port,
    pub data: PortData<'a>,
}

pub enum PortData<'a> {
    AudioInput(&'a [Data]),
    AudioOutput(RefCell<&'a mut [Data]>),
    ControlInput(&'a Data),
    ControlOutput(RefCell<&'a mut Data>),
}

unsafe impl<'a> Sync for PortData<'a> { }

impl<'a> PortConnection<'a> {
    pub fn unwrap_audio(&'a self) -> &'a [Data] {
        if let PortData::AudioInput(data) = self.data {
            data
        } else {
            panic!("PortConnection::unwrap_audio called on a non audio input port!")
        }
    }

    pub fn unwrap_audio_mut(&'a self) -> RefMut<'a, &'a mut [Data]> {
        if let PortData::AudioOutput(ref data) = self.data {
            data.borrow_mut()
        } else {
            panic!("PortConnection::unwrap_audio_mut called on a non audio output port!")
        }
    }

    pub fn unwrap_control(&'a self) -> &'a Data {
        if let PortData::ControlInput(data) = self.data {
            data
        } else {
            panic!("PortConnection::unwrap_control called on a non control input port!")
        }
    }

    pub fn unwrap_control_mut(&'a self) -> RefMut<'a, &'a mut Data> {
        if let PortData::ControlOutput(ref data) = self.data {
            data.borrow_mut()
        } else {
            panic!("PortConnection::unwrap_control called on a non control output port!")
        }
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct Properties: i32 {
        const PROP_NONE = 0;
        const PROP_REALTIME = ladspa_h::PROPERTY_REALTIME;
        const PROP_INPLACE_BROKEN = ladspa_h::PROPERTY_INPLACE_BROKEN;
        const PROP_HARD_REALTIME_CAPABLE = ladspa_h::PROPERTY_HARD_RT_CAPABLE;
    }
}

// Re-export constants for backward compatibility (e.g., ladspa::PROP_NONE)
pub const PROP_NONE: Properties = Properties::PROP_NONE;
pub const PROP_REALTIME: Properties = Properties::PROP_REALTIME;
pub const PROP_INPLACE_BROKEN: Properties = Properties::PROP_INPLACE_BROKEN;
pub const PROP_HARD_REALTIME_CAPABLE: Properties = Properties::PROP_HARD_REALTIME_CAPABLE;

pub trait Plugin {
    fn activate(&mut self) { }
    fn run<'a>(&mut self, sample_count: usize, ports: &[&'a PortConnection<'a>]);
    fn deactivate(&mut self) { }
}
