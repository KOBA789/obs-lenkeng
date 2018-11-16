use std::error;
use std::fmt;
use std::ffi::CStr;
use libturbojpeg_sys::{
    TJCS,
    tjhandle,
    TJPF_TJPF_BGRX,
    tjInitDecompress,
    tjDestroy,
    tjDecompressHeader3,
    tjDecompress2,
    tjGetErrorCode,
    tjGetErrorStr2,
};
mod tjcs {
    pub use libturbojpeg_sys::{
        TJCS_TJCS_CMYK as CMYK,
        TJCS_TJCS_GRAY as GRAY,
        TJCS_TJCS_RGB as RGB,
        TJCS_TJCS_YCCK as YCCK,
        TJCS_TJCS_YCbCr as YCbCr,
    };
}

#[allow(non_upper_case_globals)]
const tjPixelSize: [usize; 12] = [3, 3, 4, 4, 4, 4, 1, 4, 4, 4, 4, 4];

#[derive(Debug)]
pub enum ColorSpace {
    CMYK,
    GRAY,
    RGB,
    YCCK,
    YCbCr,
}
impl ColorSpace {
    fn to_tjcs(&self) -> TJCS {
        use self::ColorSpace::*;
        match self {
            CMYK => tjcs::CMYK,
            GRAY => tjcs::GRAY,
            RGB => tjcs::RGB,
            YCCK => tjcs::YCCK,
            YCbCr => tjcs::YCbCr,
        }
    }

    fn from_tjcs(tjcs: TJCS) -> ColorSpace {
        use self::ColorSpace::*;
        match tjcs {
            tjcs::CMYK => CMYK,
            tjcs::GRAY => GRAY,
            tjcs::RGB => RGB,
            tjcs::YCCK => YCCK,
            tjcs::YCbCr => YCbCr,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug)]
pub struct Header {
    pub width: i32,
    pub height: i32,
    pub subsamp: i32, // TODO: Use proper enum
    pub colorspace: ColorSpace,
}
impl Header {
    pub fn dst_size(&self) -> usize {
        let width = self.width as usize;
        let height = self.height as usize;
        let pixel_size = tjPixelSize[TJPF_TJPF_BGRX as usize];
        return width * height * pixel_size;
    }
}

#[derive(Debug)]
pub struct Error {
    pub message: String,
    pub code: i32,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl error::Error for Error {}

pub struct Decompress {
    tj: tjhandle,
}

impl Decompress {
    pub fn new() -> Option<Self> {
        let tj = unsafe { tjInitDecompress().as_mut() };
        tj.map(|tj| Decompress { tj })
    }

    pub fn decompress_header(&mut self, buf: &[u8]) -> Header {
        let mut width = 0i32;
        let mut height = 0i32;
        let mut subsamp = 0i32;
        let mut colorspace = 0i32;
        unsafe {
            tjDecompressHeader3(self.tj, buf.as_ptr(), buf.len() as u64, &mut width, &mut height, &mut subsamp, &mut colorspace);
        }
        Header {
            width,
            height,
            subsamp,
            colorspace: ColorSpace::from_tjcs(colorspace as u32),
        }
    }

    pub fn decompress(&mut self, buf: &[u8], header: &Header, dst: &mut [u8]) -> Result<(), Error> {
        assert!(header.dst_size() <= dst.len());
        let ret = unsafe {
            tjDecompress2(self.tj, buf.as_ptr(), buf.len() as u64, dst.as_mut_ptr(), header.width, 0, header.height, TJPF_TJPF_BGRX, 0)
        };
        if ret == 0 {
            Ok(())
        } else {
            Err(self.get_error())
        }
    }

    fn get_error(&mut self) -> Error {
        unsafe {
            let code = tjGetErrorCode(self.tj);
            let c_msg = tjGetErrorStr2(self.tj);
            let message = CStr::from_ptr(c_msg).to_string_lossy().to_string();
            Error { message, code }
        }
    }
}

impl Drop for Decompress {
    fn drop(&mut self) {
        unsafe {
            tjDestroy(self.tj);
        }
    }
}
