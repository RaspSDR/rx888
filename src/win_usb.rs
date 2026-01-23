//! Windows SetupAPI-based USB interface using the Cypress CyFX3 driver.
//!
//! This module provides a `WinUsb` struct that implements the `UsbInterface` trait
//! using Windows' SetupAPI and device IOCTL calls directly. It is NOT based on libusb.

#[cfg(target_os = "windows")]
mod windows_impl {
    use crate::usb_interface::UsbInterface;
    use anyhow::Result;
    use std::mem::{size_of, zeroed};
    use std::ptr;
    use std::time::Duration;

    use std::ffi::c_void;
    use winapi::ctypes::c_void as winapi_c_void;
    use winapi::shared::guiddef::GUID;
    use winapi::shared::minwindef::DWORD;
    use winapi::shared::winerror::ERROR_NO_MORE_ITEMS;
    use winapi::um::errhandlingapi::GetLastError;
    use winapi::um::fileapi::CreateFileA;
    use winapi::um::fileapi::OPEN_EXISTING;
    use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
    use winapi::um::ioapiset::DeviceIoControl;
    use winapi::um::setupapi::{
        SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsA,
        SetupDiGetDeviceInterfaceDetailA,
    };
    use winapi::um::winbase::FILE_FLAG_OVERLAPPED;
    use winapi::um::winbase::{
        FORMAT_MESSAGE_FROM_SYSTEM, FORMAT_MESSAGE_IGNORE_INSERTS, FormatMessageA,
    };
    use winapi::um::winnt::HANDLE;
    use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ, GENERIC_WRITE};

    /// Define CTL_CODE for vendor IOCTL (same as Windows kernel macro)
    const fn ctl_code(function: DWORD, method: DWORD) -> DWORD {
        (0x22 << 16) | (function << 2) | method
    }

    /// IOCTL command for EP0 control transfer (function 8, METHOD_BUFFERED)
    const IOCTL_ADAPT_SEND_EP0_CONTROL_TRANSFER: DWORD = ctl_code(8, 0); // METHOD_BUFFERED = 0

    #[repr(C, packed(1))]
    #[allow(non_snake_case)]
    struct SetupPacket {
        bmRequest: u8,
        bRequest: u8,
        wValue: u16,
        wIndex: u16,
        wLength: u16,
        ulTimeOut: u32,
    }

    #[repr(C, packed(1))]
    #[allow(non_snake_case)]
    struct SingleTransfer {
        SetupPacket: SetupPacket,
        reserved: u8,
        ucEndpointAddress: u8,
        NtStatus: u32,
        UsbdStatus: u32,
        IsoPacketOffset: u32,
        IsoPacketLength: u32,
        BufferOffset: u32,
        BufferLength: u32,
    }

    /// Convert Windows error code to human-readable string
    unsafe fn win32_error_string(code: DWORD) -> String {
        let mut buffer = [0u8; 256];
        let len = unsafe {
            FormatMessageA(
                FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
                ptr::null(),
                code,
                0,
                buffer.as_mut_ptr() as *mut i8,
                256,
                ptr::null_mut(),
            )
        };
        if len > 0 {
            let s = String::from_utf8_lossy(&buffer[..len as usize]);
            s.trim().to_string()
        } else {
            format!("Error(0x{:X})", code)
        }
    }

    /// Find the FX3 device by GUID and return its device interface name
    unsafe fn find_fx3_device_name(guid: &GUID) -> Option<String> {
        let hdev = unsafe {
            SetupDiGetClassDevsA(
                guid as *const GUID as *mut GUID,
                ptr::null(),
                ptr::null_mut(),
                winapi::um::setupapi::DIGCF_PRESENT | winapi::um::setupapi::DIGCF_DEVICEINTERFACE,
            )
        };
        if hdev == INVALID_HANDLE_VALUE {
            return None;
        }

        for index in 0..10 {
            let mut interface_data: winapi::um::setupapi::SP_DEVICE_INTERFACE_DATA =
                unsafe { zeroed() };
            interface_data.cbSize =
                size_of::<winapi::um::setupapi::SP_DEVICE_INTERFACE_DATA>() as u32;

            let ok = unsafe {
                SetupDiEnumDeviceInterfaces(
                    hdev,
                    ptr::null_mut(),
                    guid as *const GUID as *mut GUID,
                    index,
                    &mut interface_data,
                )
            };
            if ok == 0 {
                let err = unsafe { GetLastError() };
                if err == ERROR_NO_MORE_ITEMS {
                    break;
                }
                continue;
            }

            // Get the size of the path
            let mut required_len: u32 = 0;
            unsafe {
                SetupDiGetDeviceInterfaceDetailA(
                    hdev,
                    &mut interface_data,
                    ptr::null_mut(),
                    0,
                    &mut required_len,
                    ptr::null_mut(),
                )
            };

            if required_len == 0 {
                continue;
            }

            let buf = unsafe {
                libc::malloc(required_len as usize)
                    as *mut winapi::um::setupapi::SP_DEVICE_INTERFACE_DETAIL_DATA_A
            };
            if buf.is_null() {
                continue;
            }

            unsafe {
                (*buf).cbSize =
                    size_of::<winapi::um::setupapi::SP_DEVICE_INTERFACE_DETAIL_DATA_A>() as u32;
            }

            let mut devinfo: winapi::um::setupapi::SP_DEVINFO_DATA = unsafe { zeroed() };
            devinfo.cbSize = size_of::<winapi::um::setupapi::SP_DEVINFO_DATA>() as u32;

            let ok2 = unsafe {
                SetupDiGetDeviceInterfaceDetailA(
                    hdev,
                    &mut interface_data,
                    buf as *mut _,
                    required_len,
                    ptr::null_mut(),
                    &mut devinfo,
                )
            };
            if ok2 == 0 {
                unsafe { libc::free(buf as *mut _ as *mut c_void) };
                continue;
            }

            let path_ptr = unsafe { &(*buf).DevicePath[0] as *const i8 };
            let path = unsafe {
                std::ffi::CStr::from_ptr(path_ptr)
                    .to_string_lossy()
                    .into_owned()
            };
            unsafe { libc::free(buf as *mut _ as *mut c_void) };
            unsafe { SetupDiDestroyDeviceInfoList(hdev) };

            return Some(path);
        }

        unsafe { SetupDiDestroyDeviceInfoList(hdev) };
        None
    }

    /// Open a device by its interface name and return the HANDLE
    unsafe fn open_device(path: &str) -> Option<HANDLE> {
        let c_path = std::ffi::CString::new(path).ok()?;
        let handle = unsafe {
            CreateFileA(
                c_path.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                ptr::null_mut(),
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED,
                ptr::null_mut(),
            )
        };

        if handle == INVALID_HANDLE_VALUE {
            None
        } else {
            Some(handle)
        }
    }

    /// Find FX3 device by VID/PID and open it
    unsafe fn find_fx3_device_handle(_vid: u16, _pid: u16) -> Option<HANDLE> {
        // GUID from CyAPI.h: {AE18AA60-7F6A-11d4-97DD-00010229B959}
        let guid = GUID {
            Data1: 0xae18aa60,
            Data2: 0x7f6a,
            Data3: 0x11d4,
            Data4: [0x97, 0xdd, 0x00, 0x01, 0x02, 0x29, 0xb9, 0x59],
        };

        let path = unsafe { find_fx3_device_name(&guid) }?;
        unsafe { open_device(&path) }
    }

    /// Perform a vendor control transfer on endpoint 0
    unsafe fn control_transfer_ep0(
        handle: HANDLE,
        request: u8,
        addr: u32,
        write: Option<&[u8]>,
        read_buf: Option<&mut [u8]>,
    ) -> std::result::Result<usize, DWORD> {
        let write_len = write.map(|w| w.len()).unwrap_or(0);
        let read_len = read_buf.as_ref().map(|r| r.len()).unwrap_or(0);
        // For writes, we need space for header + write data
        // For reads, we need space for header + read data
        let data_len = std::cmp::max(write_len, read_len);
        let buf_len = size_of::<SingleTransfer>() + data_len;
        let mut buf = vec![0u8; buf_len];

        // Populate SINGLE_TRANSFER header
        let st_ptr = buf.as_mut_ptr() as *mut SingleTransfer;
        let direction: u8 = if read_buf.is_some() { 1 } else { 0 };
        unsafe {
            (*st_ptr).SetupPacket.bmRequest = ((direction & 0x1) << 7) | ((2u8 & 0x3) << 5);
            (*st_ptr).SetupPacket.bRequest = request;
            (*st_ptr).SetupPacket.wValue = (addr & 0xFFFF) as u16;
            (*st_ptr).SetupPacket.wIndex = ((addr >> 16) & 0xFFFF) as u16;
            (*st_ptr).SetupPacket.wLength = data_len as u16;
            (*st_ptr).SetupPacket.ulTimeOut = 5;
            (*st_ptr).ucEndpointAddress = 0x00;
            (*st_ptr).BufferOffset = size_of::<SingleTransfer>() as u32;
            (*st_ptr).BufferLength = data_len as u32;
        }

        // Copy write data after the header
        if let Some(w) = write {
            let dest = unsafe { buf.as_mut_ptr().add(size_of::<SingleTransfer>()) };
            unsafe { ptr::copy_nonoverlapping(w.as_ptr(), dest, write_len) };
        }

        let mut bytes_returned: u32 = 0;
        let ok = unsafe {
            DeviceIoControl(
                handle,
                IOCTL_ADAPT_SEND_EP0_CONTROL_TRANSFER,
                buf.as_mut_ptr() as *mut winapi_c_void,
                buf_len as u32,
                buf.as_mut_ptr() as *mut winapi_c_void,
                buf_len as u32,
                &mut bytes_returned,
                ptr::null_mut(),
            )
        };

        if ok == 0 {
            let err = unsafe { GetLastError() };
            return Err(err);
        }

        // Read response data from buffer
        if let Some(rbuf) = read_buf {
            let st = unsafe { &*(buf.as_ptr() as *const SingleTransfer) };
            let off = st.BufferOffset as usize;
            let len = std::cmp::min(rbuf.len(), st.BufferLength as usize);
            if off + len <= buf.len() {
                let src = unsafe { buf.as_ptr().add(off) };
                unsafe { ptr::copy_nonoverlapping(src, rbuf.as_mut_ptr(), len) };
                return Ok(len);
            }
        }

        Ok(write_len)
    }

    /// A thin, safe wrapper around a Win32 device HANDLE that implements
    /// the crate `UsbInterface` using the vendor IOCTL control-transfer path
    /// exposed by the CyFX3 Windows driver. This uses SetupAPI and DeviceIoControl
    /// directly; it is NOT based on libusb.
    pub struct WinUsb {
        handle: HANDLE,
    }

    impl WinUsb {
        /// Try to open the FX3 device by VID/PID and return a `WinUsb` if successful.
        pub fn open(vid: u16, pid: u16) -> Option<Self> {
            let h = unsafe { find_fx3_device_handle(vid, pid) }?;
            Some(WinUsb { handle: h })
        }

        /// Expose the raw handle for callers that need it.
        #[allow(dead_code)]
        pub fn raw_handle(&self) -> HANDLE {
            self.handle
        }
    }

    impl Drop for WinUsb {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.handle) };
        }
    }

    impl UsbInterface for WinUsb {
        fn control_write(
            &self,
            request: u8,
            value: u16,
            index: u16,
            data: &[u8],
            _timeout: Duration,
        ) -> Result<()> {
            let addr = ((index as u32) << 16) | (value as u32);
            unsafe {
                match control_transfer_ep0(self.handle, request, addr, Some(data), None) {
                    Ok(_) => Ok(()),
                    Err(code) => Err(anyhow::anyhow!(
                        "Device control_write failed: {}",
                        win32_error_string(code)
                    )),
                }
            }
        }

        fn control_read(
            &self,
            request: u8,
            value: u16,
            index: u16,
            length: u16,
            _timeout: Duration,
        ) -> Result<Vec<u8>> {
            let addr = ((index as u32) << 16) | (value as u32);
            let mut buf = vec![0u8; length as usize];
            unsafe {
                match control_transfer_ep0(self.handle, request, addr, None, Some(&mut buf)) {
                    Ok(len) => {
                        buf.truncate(len);
                        Ok(buf)
                    }
                    Err(code) => Err(anyhow::anyhow!(
                        "Device control_read failed: {}",
                        win32_error_string(code)
                    )),
                }
            }
        }
    }
}

#[cfg(target_os = "windows")]
#[allow(unused_imports)]
pub use windows_impl::WinUsb;

