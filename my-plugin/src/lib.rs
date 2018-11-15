extern crate libobs_sys;
extern crate rand;

use std::ptr::null;
use std::os::raw::c_char;
use std::mem;
use std::ffi::{CString, c_void};
use std::sync::mpsc;

use rand::Rng;

static mut OBS_MODULE_POINTER: Option<*mut libobs_sys::obs_module_t> = None;
static mut SOURCE_ID: Option<CString> = None;
static mut SOURCE_NAME: Option<CString> = None;

#[no_mangle]
pub unsafe extern "C" fn obs_module_set_pointer(module: *mut libobs_sys::obs_module_t) -> () {
    OBS_MODULE_POINTER = Some(module);
}

#[no_mangle]
pub unsafe extern "C" fn obs_module_ver() -> u32 {
    ((libobs_sys::LIBOBS_API_MAJOR_VER as u32) << 24)
    | ((libobs_sys::LIBOBS_API_MINOR_VER as u32) << 16)
    | libobs_sys::LIBOBS_API_PATCH_VER as u32
}

pub unsafe extern "C" fn source_get_name(_data: *mut c_void) -> *const c_char
{
    return SOURCE_NAME.as_ref().unwrap().as_ptr();
}

#[derive(Clone, Copy)]
struct SendSource(*mut libobs_sys::obs_source);
unsafe impl Send for SendSource {}
impl Into<*mut libobs_sys::obs_source> for SendSource {
    fn into(self) -> *mut libobs_sys::obs_source {
        self.0
    }
}
impl SendSource {
    fn output_video(&self, frame: &libobs_sys::obs_source_frame) {
        unsafe {
            libobs_sys::obs_source_output_video(self.0, frame);
        }
    }
}

fn os_gettime_ns() -> u64 {
    unsafe {
        libobs_sys::os_gettime_ns()
    }
}

fn os_sleepto_ns(dur: u64) {
    unsafe {
        libobs_sys::os_sleepto_ns(dur);
    }
}

enum Signal {
    Shutdown,
}

struct SourceData {
    chan: mpsc::SyncSender<Signal>,
}

fn render(source: SendSource, chan: mpsc::Receiver<Signal>) {
    let mut rng = rand::thread_rng();
    let mut pixels = [0u32; 20 * 20];
    let nil = (null() as *const u8) as *mut u8;
    let mut frame = libobs_sys::obs_source_frame {
        data: [pixels.as_ptr() as *mut u8, nil, nil, nil, nil, nil, nil, nil],
        linesize: [20 * 4, 0, 0, 0, 0, 0, 0, 0],
        width: 20,
        height: 20,
        format: libobs_sys::video_format_VIDEO_FORMAT_BGRX,
        ..libobs_sys::obs_source_frame::default()
    };

    while let Err(mpsc::TryRecvError::Empty) = chan.try_recv() {
        let cur_time = os_gettime_ns();
        frame.timestamp = cur_time;
        for pixel in pixels.iter_mut() {
            *pixel = rng.gen_range(0, 0xFFFFFF);
        }
        source.output_video(&frame);
        os_sleepto_ns(cur_time + 250_000_000);
    }
}

pub unsafe extern "C" fn source_create(_settings: *mut libobs_sys::obs_data, source: *mut libobs_sys::obs_source) -> *mut c_void {
    let send_source = SendSource(source);
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        render(send_source, rx);
    });
    let ptr = Box::into_raw(Box::new(SourceData { chan: tx }));
    return ptr as *mut c_void;
}

pub unsafe extern "C" fn source_destroy(data: *mut c_void) {
    let data = Box::from_raw(data as *mut SourceData);
    data.chan.send(Signal::Shutdown).ok();
}

#[no_mangle]
pub unsafe extern "C" fn obs_module_load() -> bool
{
    SOURCE_ID = Some(CString::new("noise-source").unwrap());
    SOURCE_NAME = Some(CString::new("NOISE SOURCE").unwrap());

    let source_info = libobs_sys::obs_source_info {
        id: SOURCE_ID.as_ref().unwrap().as_ptr(),
        type_: libobs_sys::obs_source_type_OBS_SOURCE_TYPE_INPUT,
        output_flags: libobs_sys::OBS_SOURCE_ASYNC_VIDEO,
        get_name: Some(source_get_name),
        create: Some(source_create),
        destroy: Some(source_destroy),
        ..libobs_sys::obs_source_info::default()
    };
    libobs_sys::obs_register_source_s(
        &source_info,
        mem::size_of::<libobs_sys::obs_source_info>()
    );
    return true;
}
