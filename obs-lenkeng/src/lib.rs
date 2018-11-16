extern crate libobs_sys;
extern crate image;

use image::GenericImageView;
use std::ptr::null;
use std::os::raw::c_char;
use std::mem;
use std::ffi::{CString, c_void};
use std::sync::mpsc;
use std::net::UdpSocket;
use std::net::Ipv4Addr;

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

enum Signal {
    Shutdown,
}

struct SourceData {
    chan: mpsc::SyncSender<Signal>,
}

const WIDTH: usize = 1920;
const HEIGHT: usize = 1080;
const PACKET_SIZE: usize = 1024;
const MAX_CHUNK: usize = 1000;

fn render(source: SendSource, chan: mpsc::Receiver<Signal>) {
    let nil = (null() as *const u8) as *mut u8;
    let pixels: Vec<u32> = vec![0; WIDTH * HEIGHT];
    let mut frame = libobs_sys::obs_source_frame {
        data: [pixels.as_ptr() as *mut u8, nil, nil, nil, nil, nil, nil, nil],
        linesize: [WIDTH as u32 * 4, 0, 0, 0, 0, 0, 0, 0],
        width: WIDTH as u32,
        height: HEIGHT as u32,
        format: libobs_sys::video_format_VIDEO_FORMAT_BGRX,
        ..libobs_sys::obs_source_frame::default()
    };

    let socket = UdpSocket::bind("0.0.0.0:2068").expect("failed to bind to address");
    let membership: Ipv4Addr = "226.2.2.2".parse().unwrap();
    let ifaddr: Ipv4Addr = "192.168.168.123".parse().unwrap();
    socket.join_multicast_v4(&membership, &ifaddr).expect("failed to join to multicast group");
    let mut buf: Vec<u8> = Vec::with_capacity(PACKET_SIZE * MAX_CHUNK);
    let mut chunk_buf: Vec<u8> = vec![0; PACKET_SIZE];

    loop {
        socket.recv(&mut chunk_buf).expect("failed to read from socket");
        //let frame_n = (chunk_buf[0] as u16) * 0xFF + chunk_buf[1] as u16;
        let part_n = (chunk_buf[2] as u16) * 0xFF + chunk_buf[3] as u16;

        if part_n == 0 {
            buf.clear();
            frame.timestamp = os_gettime_ns();
        }

        buf.extend(&chunk_buf[4..]);

        if part_n > 0x4000 {
            if let Ok(_) = chan.try_recv() {
                break;
            }
            let dec_ret = image::load_from_memory_with_format(&buf, image::JPEG);
            match dec_ret {
                Ok(img) => {
                    let pixels = img.to_bgra().into_raw();
                    frame.width = img.width();
                    frame.height = img.height();
                    frame.linesize[0] = img.width() * 4;
                    frame.data[0] = pixels.as_ptr() as *mut u8;
                },
                Err(err) => {
                    println!("{}", err);
                }
            }
            source.output_video(&frame);
        }
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
