extern crate libobs_sys;
extern crate libturbojpeg_sys;
extern crate net2;
extern crate worker_sentinel;

mod turbojpeg;

use std::ptr::null;
use std::os::raw::c_char;
use std::mem;
use std::ffi::{c_void, CStr};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::net::{UdpSocket, Ipv4Addr};
use std::thread;
use std::time;
use net2::unix::UnixUdpBuilderExt;
use worker_sentinel::Work;

static mut OBS_MODULE_POINTER: Option<*mut libobs_sys::obs_module_t> = None;
const SOURCE_ID: &[u8] = b"lenkeng\0";
const SOURCE_NAME: &[u8] = b"LENKENG\0";

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

unsafe extern "C" fn source_get_name(_data: *mut c_void) -> *const c_char {
    return SOURCE_NAME.as_ptr() as *const c_char;
}

#[derive(Clone, Copy)]
struct SendSource(*mut libobs_sys::obs_source);
unsafe impl Sync for SendSource {}
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

struct SourceData {
    is_destroyed: Arc<AtomicBool>,
}

const PACKET_SIZE: usize = 1024;
const MAX_CHUNK: usize = 1000;

struct RenderWork {
    ifaddr_string: String,
    source: SendSource,
    is_destroyed: Arc<AtomicBool>,
}
impl RenderWork {
    fn render(self, socket: UdpSocket) {
        let nil = (null() as *const u8) as *mut u8;
        let mut pixels = Vec::<u8>::new();
        let mut frame = libobs_sys::obs_source_frame {
            data: [pixels.as_ptr() as *mut u8, nil, nil, nil, nil, nil, nil, nil],
            linesize: [0, 0, 0, 0, 0, 0, 0, 0],
            width: 0,
            height: 0,
            format: libobs_sys::video_format_VIDEO_FORMAT_BGRX,
            ..libobs_sys::obs_source_frame::default()
        };

        let mut jpeg_buf = Vec::<u8>::with_capacity(PACKET_SIZE * MAX_CHUNK);
        let mut chunk_buf = vec![0u8; PACKET_SIZE];
        let mut dec = turbojpeg::Decompress::new().unwrap();

        loop {
            socket.recv(&mut chunk_buf).expect("failed to read from socket");
            //let frame_n = (chunk_buf[0] as u16) * 0xFF + chunk_buf[1] as u16;
            let part_n = (chunk_buf[2] as u16) * 0xFF + chunk_buf[3] as u16;

            if part_n == 0 {
                jpeg_buf.clear();
                frame.timestamp = os_gettime_ns();
            }

            jpeg_buf.extend_from_slice(&chunk_buf[4..]);

            if part_n > 0x4000 {
                if self.is_destroyed.load(Ordering::SeqCst) {
                    break;
                }
                let header = dec.decompress_header(&jpeg_buf);
                if header.dst_size() > pixels.len() {
                    pixels.resize(header.dst_size(), 0);
                    frame.data[0] = pixels.as_ptr() as *mut u8;
                }
                let dec_ret = dec.decompress(&jpeg_buf, &header, pixels.as_mut_slice());
                match dec_ret {
                    Ok(_) => {
                        frame.width = header.width as u32;
                        frame.height = header.height as u32;
                        frame.linesize[0] = header.width as u32 * 4;
                    },
                    Err(err) => {
                        println!("{}", err);
                    }
                }
                self.source.output_video(&frame);
            }
        }
    }

    fn create_socket(&self) -> Option<UdpSocket> {
        let socket = net2::UdpBuilder::new_v4().ok()?
            .reuse_port(true).ok()?
            .bind("0.0.0.0:2068").ok()?;
        let membership: Ipv4Addr = "226.2.2.2".parse().unwrap();
        let ifaddr: Ipv4Addr = self.ifaddr_string.parse().ok()?;
        socket.join_multicast_v4(&membership, &ifaddr).ok()?;
        Some(socket)
    }

    fn try_to_create_socket(&self) -> UdpSocket {
        loop {
            if let Some(socket) = self.create_socket() {
                return socket;
            }
            thread::sleep(time::Duration::from_secs(1));
        }
    }
}
impl Work for RenderWork {
    fn work(self) -> Option<Self> {
        let socket = self.try_to_create_socket();
        self.render(socket);
        None
    }
}

unsafe extern "C" fn source_create(settings_raw: *mut libobs_sys::obs_data, source: *mut libobs_sys::obs_source) -> *mut c_void {
    let send_source = SendSource(source);
    let is_destroyed = Arc::new(AtomicBool::new(false));
    let ifaddr_ptr = libobs_sys::obs_data_get_string(settings_raw, S_IFADDR.as_ptr() as *const i8);
    let ifaddr_cstr = CStr::from_ptr(ifaddr_ptr);
    let ifaddr_string = ifaddr_cstr.to_string_lossy().to_string();

    let is_destroyed2 = is_destroyed.clone();
    worker_sentinel::spawn(1, move || {
        RenderWork {
            ifaddr_string: ifaddr_string.clone(),
            source: send_source.clone(),
            is_destroyed: is_destroyed2.clone(),
        }
    });

    let ptr = Box::into_raw(Box::new(SourceData { is_destroyed }));
    println!("create LENKENG");
    return ptr as *mut c_void;
}

const S_IFADDR: &[u8] = b"ifaddr\0";
const S_IFADDR_DEFAULT: &[u8] = b"\0";
unsafe extern "C" fn source_get_defaults(settings_raw: *mut libobs_sys::obs_data) {
    libobs_sys::obs_data_set_default_string(settings_raw, S_IFADDR.as_ptr() as *const i8, S_IFADDR_DEFAULT.as_ptr() as *const i8);
}

const P_IFADDR_DESCRIPTION: &[u8] = b"Interface Address\n";
unsafe extern "C" fn source_get_properties(_data: *mut c_void) -> *mut libobs_sys::obs_properties {
    let ppts = libobs_sys::obs_properties_create();
    libobs_sys::obs_properties_add_text(ppts, S_IFADDR.as_ptr() as *const i8, P_IFADDR_DESCRIPTION.as_ptr() as *const i8, libobs_sys::obs_text_type_OBS_TEXT_DEFAULT);
    return ppts;
}

unsafe extern "C" fn source_destroy(data: *mut c_void) {
    let data = Box::from_raw(data as *mut SourceData);
    data.is_destroyed.store(true, Ordering::SeqCst);
    println!("destroy LENKENG");
}

#[no_mangle]
pub unsafe extern "C" fn obs_module_load() -> bool
{
    let source_info = libobs_sys::obs_source_info {
        id: SOURCE_ID.as_ptr() as *const c_char,
        type_: libobs_sys::obs_source_type_OBS_SOURCE_TYPE_INPUT,
        output_flags: libobs_sys::OBS_SOURCE_ASYNC_VIDEO,
        get_name: Some(source_get_name),
        create: Some(source_create),
        destroy: Some(source_destroy),
        get_defaults: Some(source_get_defaults),
        get_properties: Some(source_get_properties),
        ..libobs_sys::obs_source_info::default()
    };
    libobs_sys::obs_register_source_s(
        &source_info,
        mem::size_of::<libobs_sys::obs_source_info>()
    );
    return true;
}
