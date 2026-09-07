pub(crate) const USB_ENDPOINT_HINT: &str = "usb";

pub(crate) const fn host_available() -> bool {
    cfg!(target_os = "linux")
}

#[cfg(target_os = "android")]
pub(crate) use android::{from_accessory_fd, AndroidUsbStream};
#[cfg(target_os = "linux")]
pub(crate) use linux::open_host_accessory;

#[cfg(target_os = "linux")]
mod linux {
    use std::{
        fmt,
        io::{self, Read, Write},
        os::raw::{c_int, c_uchar, c_uint, c_ushort},
        ptr,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        thread,
        time::{Duration, Instant},
    };

    use crate::error::CommandError;

    use super::super::{
        cancellation::{SyncAbort, SyncAbortSource, SyncCancellationSignal},
        transport::SyncIo,
    };

    const GOOGLE_VENDOR_ID: u16 = 0x18d1;
    const AOA_PRODUCT_ID: u16 = 0x2d00;
    const AOA_ADB_PRODUCT_ID: u16 = 0x2d01;
    const AOA_GET_PROTOCOL: u8 = 51;
    const AOA_SEND_STRING: u8 = 52;
    const AOA_START: u8 = 53;
    const USB_DIR_IN_VENDOR: u8 = 0xc0;
    const USB_DIR_OUT_VENDOR: u8 = 0x40;
    const LIBUSB_TRANSFER_TYPE_MASK: u8 = 0x03;
    const LIBUSB_TRANSFER_TYPE_BULK: u8 = 0x02;
    const LIBUSB_ENDPOINT_IN: u8 = 0x80;
    const LIBUSB_ERROR_ACCESS: c_int = -3;
    const LIBUSB_ERROR_NO_DEVICE: c_int = -4;
    const LIBUSB_ERROR_TIMEOUT: c_int = -7;
    const LIBUSB_ERROR_INTERRUPTED: c_int = -10;
    const LIBUSB_ERROR_NOT_SUPPORTED: c_int = -12;
    const CONTROL_TIMEOUT_MS: c_uint = 750;
    const TRANSFER_SLICE_TIMEOUT_MS: c_uint = 350;
    const REENUMERATION_POLL: Duration = Duration::from_millis(150);
    const DEVICE_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(12);

    // Restrict AOA probing to well-known Android OEM USB vendor IDs instead of
    // sending Android vendor requests to unrelated USB peripherals. Interface
    // permissions are installed by the Linux setup/package integration.
    const ANDROID_VENDOR_IDS: &[u16] = &[
        0x0502, // Acer
        0x04e8, // Samsung
        0x0b05, // ASUS
        0x0bb4, // HTC
        0x0fce, // Sony
        0x1004, // LG
        0x12d1, // Huawei
        0x17ef, // Lenovo
        0x18d1, // Google / Pixel
        0x19d2, // ZTE
        0x22b8, // Motorola
        0x22d9, // OPPO
        0x2717, // Xiaomi
        0x2a70, // OnePlus
        0x2d95, // vivo
    ];

    #[repr(C)]
    struct LibUsbContext {
        _private: [u8; 0],
    }

    #[repr(C)]
    struct LibUsbDevice {
        _private: [u8; 0],
    }

    #[repr(C)]
    struct LibUsbDeviceHandle {
        _private: [u8; 0],
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct LibUsbDeviceDescriptor {
        b_length: c_uchar,
        b_descriptor_type: c_uchar,
        bcd_usb: c_ushort,
        b_device_class: c_uchar,
        b_device_sub_class: c_uchar,
        b_device_protocol: c_uchar,
        b_max_packet_size_0: c_uchar,
        id_vendor: c_ushort,
        id_product: c_ushort,
        bcd_device: c_ushort,
        i_manufacturer: c_uchar,
        i_product: c_uchar,
        i_serial_number: c_uchar,
        b_num_configurations: c_uchar,
    }

    #[repr(C)]
    struct LibUsbEndpointDescriptor {
        b_length: c_uchar,
        b_descriptor_type: c_uchar,
        b_endpoint_address: c_uchar,
        bm_attributes: c_uchar,
        w_max_packet_size: c_ushort,
        b_interval: c_uchar,
        b_refresh: c_uchar,
        b_synch_address: c_uchar,
        extra: *const c_uchar,
        extra_length: c_int,
    }

    #[repr(C)]
    struct LibUsbInterfaceDescriptor {
        b_length: c_uchar,
        b_descriptor_type: c_uchar,
        b_interface_number: c_uchar,
        b_alternate_setting: c_uchar,
        b_num_endpoints: c_uchar,
        b_interface_class: c_uchar,
        b_interface_sub_class: c_uchar,
        b_interface_protocol: c_uchar,
        i_interface: c_uchar,
        endpoint: *const LibUsbEndpointDescriptor,
        extra: *const c_uchar,
        extra_length: c_int,
    }

    #[repr(C)]
    struct LibUsbInterface {
        altsetting: *const LibUsbInterfaceDescriptor,
        num_altsetting: c_int,
    }

    #[repr(C)]
    struct LibUsbConfigDescriptor {
        b_length: c_uchar,
        b_descriptor_type: c_uchar,
        w_total_length: c_ushort,
        b_num_interfaces: c_uchar,
        b_configuration_value: c_uchar,
        i_configuration: c_uchar,
        bm_attributes: c_uchar,
        max_power: c_uchar,
        interface: *const LibUsbInterface,
        extra: *const c_uchar,
        extra_length: c_int,
    }

    #[link(name = "libusb-1.0.so.0", kind = "dylib", modifiers = "+verbatim")]
    unsafe extern "C" {
        fn libusb_init(context: *mut *mut LibUsbContext) -> c_int;
        fn libusb_exit(context: *mut LibUsbContext);
        fn libusb_get_device_list(
            context: *mut LibUsbContext,
            list: *mut *mut *mut LibUsbDevice,
        ) -> isize;
        fn libusb_free_device_list(list: *mut *mut LibUsbDevice, unref_devices: c_int);
        fn libusb_get_device_descriptor(
            device: *mut LibUsbDevice,
            descriptor: *mut LibUsbDeviceDescriptor,
        ) -> c_int;
        fn libusb_open(device: *mut LibUsbDevice, handle: *mut *mut LibUsbDeviceHandle) -> c_int;
        fn libusb_close(handle: *mut LibUsbDeviceHandle);
        fn libusb_control_transfer(
            handle: *mut LibUsbDeviceHandle,
            request_type: c_uchar,
            request: c_uchar,
            value: c_ushort,
            index: c_ushort,
            data: *mut c_uchar,
            length: c_ushort,
            timeout: c_uint,
        ) -> c_int;
        fn libusb_get_active_config_descriptor(
            device: *mut LibUsbDevice,
            config: *mut *const LibUsbConfigDescriptor,
        ) -> c_int;
        fn libusb_free_config_descriptor(config: *const LibUsbConfigDescriptor);
        fn libusb_set_auto_detach_kernel_driver(
            handle: *mut LibUsbDeviceHandle,
            enable: c_int,
        ) -> c_int;
        fn libusb_claim_interface(handle: *mut LibUsbDeviceHandle, interface: c_int) -> c_int;
        fn libusb_release_interface(handle: *mut LibUsbDeviceHandle, interface: c_int) -> c_int;
        fn libusb_set_interface_alt_setting(
            handle: *mut LibUsbDeviceHandle,
            interface: c_int,
            alternate_setting: c_int,
        ) -> c_int;
        fn libusb_bulk_transfer(
            handle: *mut LibUsbDeviceHandle,
            endpoint: c_uchar,
            data: *mut c_uchar,
            length: c_int,
            transferred: *mut c_int,
            timeout: c_uint,
        ) -> c_int;
        fn libusb_get_string_descriptor_ascii(
            handle: *mut LibUsbDeviceHandle,
            descriptor_index: c_uchar,
            data: *mut c_uchar,
            length: c_int,
        ) -> c_int;
    }

    struct ContextGuard(*mut LibUsbContext);

    impl ContextGuard {
        fn open() -> Result<Self, CommandError> {
            let mut context = ptr::null_mut();
            let status = unsafe { libusb_init(&mut context) };
            if status != 0 || context.is_null() {
                return Err(usb_command_error(
                    "device_sync_usb_unavailable",
                    format!(
                        "The Linux USB subsystem could not initialize (libusb status {status})."
                    ),
                ));
            }
            Ok(Self(context))
        }
    }

    impl Drop for ContextGuard {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { libusb_exit(self.0) };
            }
        }
    }

    struct HandleGuard(*mut LibUsbDeviceHandle);

    impl HandleGuard {
        fn into_raw(mut self) -> *mut LibUsbDeviceHandle {
            let value = self.0;
            self.0 = ptr::null_mut();
            value
        }
    }

    impl Drop for HandleGuard {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { libusb_close(self.0) };
            }
        }
    }

    struct DeviceList {
        raw: *mut *mut LibUsbDevice,
        count: usize,
    }

    impl DeviceList {
        fn load(context: *mut LibUsbContext) -> Result<Self, CommandError> {
            let mut raw = ptr::null_mut();
            let count = unsafe { libusb_get_device_list(context, &mut raw) };
            if count < 0 || raw.is_null() {
                return Err(usb_command_error(
                    "device_sync_usb_unavailable",
                    format!("The Linux USB device list could not be read (libusb status {count})."),
                ));
            }
            Ok(Self {
                raw,
                count: usize::try_from(count).unwrap_or_default(),
            })
        }

        fn devices(&self) -> impl Iterator<Item = *mut LibUsbDevice> + '_ {
            (0..self.count).filter_map(|index| {
                let value = unsafe { *self.raw.add(index) };
                (!value.is_null()).then_some(value)
            })
        }
    }

    impl Drop for DeviceList {
        fn drop(&mut self) {
            if !self.raw.is_null() {
                unsafe { libusb_free_device_list(self.raw, 1) };
            }
        }
    }

    #[derive(Debug)]
    struct UsbAbort {
        canceled: Arc<AtomicBool>,
    }

    impl SyncAbort for UsbAbort {
        fn abort(&self) {
            self.canceled.store(true, Ordering::SeqCst);
        }
    }

    pub(crate) struct HostUsbStream {
        _context: ContextGuard,
        handle: *mut LibUsbDeviceHandle,
        interface_number: c_int,
        input_endpoint: u8,
        output_endpoint: u8,
        canceled: Arc<AtomicBool>,
    }

    // The libusb handle is created and consumed by one synchronization worker.
    // Cancellation shares only the atomic flag, never the native handle itself.
    unsafe impl Send for HostUsbStream {}

    impl fmt::Debug for HostUsbStream {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter
                .debug_struct("HostUsbStream")
                .field("interface_number", &self.interface_number)
                .field("input_endpoint", &self.input_endpoint)
                .field("output_endpoint", &self.output_endpoint)
                .finish_non_exhaustive()
        }
    }

    impl Drop for HostUsbStream {
        fn drop(&mut self) {
            if !self.handle.is_null() {
                unsafe {
                    let _ = libusb_release_interface(self.handle, self.interface_number);
                    libusb_close(self.handle);
                }
                self.handle = ptr::null_mut();
            }
        }
    }

    impl SyncAbortSource for HostUsbStream {
        fn sync_abort_handle(&self) -> Result<Arc<dyn SyncAbort>, CommandError> {
            Ok(Arc::new(UsbAbort {
                canceled: Arc::clone(&self.canceled),
            }))
        }
    }

    impl SyncIo for HostUsbStream {
        fn prepare_for_sync(&self, _timeout: Duration) -> Result<(), CommandError> {
            Ok(())
        }
    }

    impl Read for HostUsbStream {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            if buffer.is_empty() {
                return Ok(0);
            }
            let length = c_int::try_from(buffer.len()).unwrap_or(c_int::MAX);
            loop {
                ensure_not_canceled(&self.canceled)?;
                let mut transferred = 0;
                let status = unsafe {
                    libusb_bulk_transfer(
                        self.handle,
                        self.input_endpoint,
                        buffer.as_mut_ptr(),
                        length,
                        &mut transferred,
                        TRANSFER_SLICE_TIMEOUT_MS,
                    )
                };
                if transferred > 0 {
                    return Ok(usize::try_from(transferred).unwrap_or_default());
                }
                match status {
                    0 => return Ok(0),
                    LIBUSB_ERROR_TIMEOUT | LIBUSB_ERROR_INTERRUPTED => continue,
                    LIBUSB_ERROR_NO_DEVICE => {
                        return Err(io::Error::new(
                            io::ErrorKind::BrokenPipe,
                            "The Android USB device was disconnected.",
                        ));
                    }
                    code => return Err(libusb_io_error("read from", code)),
                }
            }
        }
    }

    impl Write for HostUsbStream {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            if buffer.is_empty() {
                return Ok(0);
            }
            let length = c_int::try_from(buffer.len()).unwrap_or(c_int::MAX);
            loop {
                ensure_not_canceled(&self.canceled)?;
                let mut transferred = 0;
                let status = unsafe {
                    libusb_bulk_transfer(
                        self.handle,
                        self.output_endpoint,
                        buffer.as_ptr().cast_mut(),
                        length,
                        &mut transferred,
                        TRANSFER_SLICE_TIMEOUT_MS,
                    )
                };
                if transferred > 0 {
                    return Ok(usize::try_from(transferred).unwrap_or_default());
                }
                match status {
                    0 => return Ok(0),
                    LIBUSB_ERROR_TIMEOUT | LIBUSB_ERROR_INTERRUPTED => continue,
                    LIBUSB_ERROR_NO_DEVICE => {
                        return Err(io::Error::new(
                            io::ErrorKind::BrokenPipe,
                            "The Android USB device was disconnected.",
                        ));
                    }
                    code => return Err(libusb_io_error("write to", code)),
                }
            }
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    pub(crate) fn open_host_accessory(
        session_id: &str,
        cancellation: &SyncCancellationSignal,
        invitation_deadline: Instant,
    ) -> Result<HostUsbStream, CommandError> {
        cancellation.check()?;
        let context = ContextGuard::open()?;
        if let Some(opened) = open_accessory_device(context.0, session_id)? {
            return opened.into_stream(context);
        }

        switch_one_android_device(context.0, session_id)?;
        let discovery_deadline = Instant::now()
            .checked_add(DEVICE_DISCOVERY_TIMEOUT)
            .map(|value| value.min(invitation_deadline))
            .unwrap_or(invitation_deadline);
        loop {
            cancellation.check()?;
            if Instant::now() >= discovery_deadline {
                return Err(usb_command_error(
                    "device_sync_usb_device_not_ready",
                    "The connected Android phone did not enter ostudy USB accessory mode. Reconnect the data cable and try USB Sync again.",
                ));
            }
            if let Some(opened) = open_accessory_device(context.0, session_id)? {
                return opened.into_stream(context);
            }
            thread::sleep(REENUMERATION_POLL);
        }
    }

    struct OpenedAccessory {
        handle: HandleGuard,
        interface_number: c_int,
        alternate_setting: c_int,
        input_endpoint: u8,
        output_endpoint: u8,
    }

    impl OpenedAccessory {
        fn into_stream(self, context: ContextGuard) -> Result<HostUsbStream, CommandError> {
            let detach = unsafe { libusb_set_auto_detach_kernel_driver(self.handle.0, 1) };
            if detach != 0 && detach != LIBUSB_ERROR_NOT_SUPPORTED {
                return Err(libusb_command_error(
                    "device_sync_usb_open_failed",
                    "configure kernel-driver detachment",
                    detach,
                ));
            }
            // AOA has already re-enumerated with its active configuration. Do
            // not reset the whole device here: PID 0x2d01 exposes ADB as a
            // sibling interface, and libusb_set_configuration would disrupt
            // that interface even though oStudy needs only the accessory bulk
            // interface below.
            let claim = unsafe { libusb_claim_interface(self.handle.0, self.interface_number) };
            if claim != 0 {
                return Err(libusb_command_error(
                    "device_sync_usb_open_failed",
                    "claim the Android accessory interface",
                    claim,
                ));
            }
            if self.alternate_setting != 0 {
                let alternate = unsafe {
                    libusb_set_interface_alt_setting(
                        self.handle.0,
                        self.interface_number,
                        self.alternate_setting,
                    )
                };
                if alternate != 0 {
                    unsafe {
                        let _ = libusb_release_interface(self.handle.0, self.interface_number);
                    }
                    return Err(libusb_command_error(
                        "device_sync_usb_open_failed",
                        "select the Android accessory alternate interface",
                        alternate,
                    ));
                }
            }
            Ok(HostUsbStream {
                _context: context,
                handle: self.handle.into_raw(),
                interface_number: self.interface_number,
                input_endpoint: self.input_endpoint,
                output_endpoint: self.output_endpoint,
                canceled: Arc::new(AtomicBool::new(false)),
            })
        }
    }

    fn open_accessory_device(
        context: *mut LibUsbContext,
        preferred_serial: &str,
    ) -> Result<Option<OpenedAccessory>, CommandError> {
        let list = DeviceList::load(context)?;
        let mut fallbacks = Vec::new();
        for device in list.devices() {
            let descriptor = device_descriptor(device)?;
            if descriptor.id_vendor != GOOGLE_VENDOR_ID
                || !matches!(descriptor.id_product, AOA_PRODUCT_ID | AOA_ADB_PRODUCT_ID)
            {
                continue;
            }
            let handle = open_device(device)?;
            let serial = read_ascii_string(handle.0, descriptor.i_serial_number);
            let endpoints = accessory_endpoints(device)?;
            let opened = OpenedAccessory {
                handle,
                interface_number: endpoints.0,
                alternate_setting: endpoints.1,
                input_endpoint: endpoints.2,
                output_endpoint: endpoints.3,
            };
            if serial.as_deref() == Some(preferred_serial) {
                return Ok(Some(opened));
            }
            fallbacks.push(opened);
        }
        match fallbacks.len() {
            0 => Ok(None),
            1 => Ok(fallbacks.pop()),
            _ => Err(usb_command_error(
                "device_sync_usb_multiple_devices",
                "More than one Android phone is already connected in USB accessory mode. Disconnect the extra phone and try USB Sync again.",
            )),
        }
    }

    fn switch_one_android_device(
        context: *mut LibUsbContext,
        session_id: &str,
    ) -> Result<(), CommandError> {
        let list = DeviceList::load(context)?;
        let mut supported = Vec::new();
        let mut access_denied = false;
        for device in list.devices() {
            let descriptor = device_descriptor(device)?;
            if descriptor.id_vendor == GOOGLE_VENDOR_ID
                && matches!(descriptor.id_product, AOA_PRODUCT_ID | AOA_ADB_PRODUCT_ID)
            {
                continue;
            }
            if !ANDROID_VENDOR_IDS.contains(&descriptor.id_vendor) {
                continue;
            }
            let handle = match open_device(device) {
                Ok(handle) => handle,
                Err(error) if error.code == "device_sync_usb_access_denied" => {
                    access_denied = true;
                    continue;
                }
                Err(_) => continue,
            };
            if query_aoa_protocol(handle.0).unwrap_or(0) > 0 {
                supported.push(handle);
            }
        }
        if supported.len() > 1 {
            return Err(usb_command_error(
                "device_sync_usb_multiple_devices",
                "More than one Android phone is connected. Leave only the phone you want to synchronize attached, then click USB Sync again.",
            ));
        }
        let Some(handle) = supported.pop() else {
            return Err(usb_command_error(
                if access_denied {
                    "device_sync_usb_access_denied"
                } else {
                    "device_sync_usb_device_not_found"
                },
                if access_denied {
                    "ostudy can see the attached Android phone but Linux has not granted USB access to this application. Repair or reinstall ostudy's Linux USB integration, reconnect the cable, and try again."
                } else {
                    "No compatible Android phone was found on a data-capable USB connection. Connect one phone and click USB Sync again."
                },
            ));
        };

        send_aoa_string(handle.0, 0, "oStudy")?;
        send_aoa_string(handle.0, 1, "oStudy USB Sync")?;
        send_aoa_string(
            handle.0,
            2,
            "Encrypted local-first oStudy device synchronization",
        )?;
        send_aoa_string(handle.0, 3, "1")?;
        send_aoa_string(handle.0, 4, "https://github.com/jferments/ostudy")?;
        send_aoa_string(handle.0, 5, session_id)?;
        let status = unsafe {
            libusb_control_transfer(
                handle.0,
                USB_DIR_OUT_VENDOR,
                AOA_START,
                0,
                0,
                ptr::null_mut(),
                0,
                CONTROL_TIMEOUT_MS,
            )
        };
        if status < 0 {
            return Err(libusb_command_error(
                "device_sync_usb_mode_failed",
                "start Android accessory mode",
                status,
            ));
        }
        Ok(())
    }

    fn query_aoa_protocol(handle: *mut LibUsbDeviceHandle) -> Result<u16, CommandError> {
        let mut bytes = [0_u8; 2];
        let status = unsafe {
            libusb_control_transfer(
                handle,
                USB_DIR_IN_VENDOR,
                AOA_GET_PROTOCOL,
                0,
                0,
                bytes.as_mut_ptr(),
                2,
                CONTROL_TIMEOUT_MS,
            )
        };
        if status != 2 {
            return Err(libusb_command_error(
                "device_sync_usb_not_android",
                "query Android Open Accessory support",
                status,
            ));
        }
        Ok(u16::from_le_bytes(bytes))
    }

    fn send_aoa_string(
        handle: *mut LibUsbDeviceHandle,
        index: u16,
        value: &str,
    ) -> Result<(), CommandError> {
        let mut payload = value.as_bytes().to_vec();
        if payload.len() >= 255 {
            payload.truncate(254);
        }
        payload.push(0);
        let length = u16::try_from(payload.len()).unwrap_or(255);
        let status = unsafe {
            libusb_control_transfer(
                handle,
                USB_DIR_OUT_VENDOR,
                AOA_SEND_STRING,
                0,
                index,
                payload.as_mut_ptr(),
                length,
                CONTROL_TIMEOUT_MS,
            )
        };
        if status != c_int::from(length) {
            return Err(libusb_command_error(
                "device_sync_usb_mode_failed",
                "identify ostudy to the Android accessory service",
                status,
            ));
        }
        Ok(())
    }

    fn device_descriptor(
        device: *mut LibUsbDevice,
    ) -> Result<LibUsbDeviceDescriptor, CommandError> {
        let mut descriptor = LibUsbDeviceDescriptor::default();
        let status = unsafe { libusb_get_device_descriptor(device, &mut descriptor) };
        if status != 0 {
            return Err(libusb_command_error(
                "device_sync_usb_unavailable",
                "read a USB device descriptor",
                status,
            ));
        }
        Ok(descriptor)
    }

    fn open_device(device: *mut LibUsbDevice) -> Result<HandleGuard, CommandError> {
        let mut handle = ptr::null_mut();
        let status = unsafe { libusb_open(device, &mut handle) };
        if status == LIBUSB_ERROR_ACCESS {
            return Err(usb_command_error(
                "device_sync_usb_access_denied",
                "Linux denied access to the connected USB device.",
            ));
        }
        if status != 0 || handle.is_null() {
            return Err(libusb_command_error(
                "device_sync_usb_open_failed",
                "open the USB device",
                status,
            ));
        }
        Ok(HandleGuard(handle))
    }

    fn accessory_endpoints(
        device: *mut LibUsbDevice,
    ) -> Result<(c_int, c_int, u8, u8), CommandError> {
        let mut config = ptr::null();
        let status = unsafe { libusb_get_active_config_descriptor(device, &mut config) };
        if status != 0 || config.is_null() {
            return Err(libusb_command_error(
                "device_sync_usb_open_failed",
                "read the Android accessory interface",
                status,
            ));
        }
        let result = unsafe {
            let config_ref = &*config;
            let first_interface = if config_ref.b_num_interfaces == 0 {
                None
            } else {
                Some(&*config_ref.interface)
            };
            first_interface.and_then(|interface| {
                if interface.num_altsetting <= 0 || interface.altsetting.is_null() {
                    return None;
                }
                let alternate = &*interface.altsetting;
                let endpoints = std::slice::from_raw_parts(
                    alternate.endpoint,
                    usize::from(alternate.b_num_endpoints),
                );
                let input = endpoints.iter().find(|endpoint| {
                    endpoint.bm_attributes & LIBUSB_TRANSFER_TYPE_MASK == LIBUSB_TRANSFER_TYPE_BULK
                        && endpoint.b_endpoint_address & LIBUSB_ENDPOINT_IN != 0
                });
                let output = endpoints.iter().find(|endpoint| {
                    endpoint.bm_attributes & LIBUSB_TRANSFER_TYPE_MASK == LIBUSB_TRANSFER_TYPE_BULK
                        && endpoint.b_endpoint_address & LIBUSB_ENDPOINT_IN == 0
                });
                input.zip(output).map(|(input, output)| {
                    (
                        c_int::from(alternate.b_interface_number),
                        c_int::from(alternate.b_alternate_setting),
                        input.b_endpoint_address,
                        output.b_endpoint_address,
                    )
                })
            })
        };
        unsafe { libusb_free_config_descriptor(config) };
        result.ok_or_else(|| {
            usb_command_error(
                "device_sync_usb_open_failed",
                "The Android accessory did not expose the expected bidirectional bulk USB interface.",
            )
        })
    }

    fn read_ascii_string(handle: *mut LibUsbDeviceHandle, index: u8) -> Option<String> {
        if index == 0 {
            return None;
        }
        let mut buffer = [0_u8; 256];
        let count = unsafe {
            libusb_get_string_descriptor_ascii(
                handle,
                index,
                buffer.as_mut_ptr(),
                c_int::try_from(buffer.len()).unwrap_or(256),
            )
        };
        if count <= 0 {
            return None;
        }
        String::from_utf8(buffer[..usize::try_from(count).ok()?].to_vec()).ok()
    }

    fn ensure_not_canceled(canceled: &AtomicBool) -> io::Result<()> {
        if canceled.load(Ordering::SeqCst) {
            Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "USB synchronization was canceled.",
            ))
        } else {
            Ok(())
        }
    }

    fn libusb_io_error(operation: &str, status: c_int) -> io::Error {
        io::Error::new(
            if status == LIBUSB_ERROR_NO_DEVICE {
                io::ErrorKind::BrokenPipe
            } else {
                io::ErrorKind::Other
            },
            format!("Could not {operation} the Android USB accessory (libusb status {status})."),
        )
    }

    fn libusb_command_error(code: &str, operation: &str, status: c_int) -> CommandError {
        usb_command_error(
            code,
            format!("ostudy could not {operation} (libusb status {status})."),
        )
    }

    fn usb_command_error(code: &str, message: impl Into<String>) -> CommandError {
        CommandError::new(code, message)
    }
}

#[cfg(target_os = "android")]
mod android {
    use std::{
        fmt,
        io::{self, Read, Write},
        os::raw::{c_int, c_short, c_void},
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        time::Duration,
    };

    use crate::error::CommandError;

    use super::super::{
        cancellation::{SyncAbort, SyncAbortSource},
        transport::SyncIo,
    };

    const POLLIN: c_short = 0x0001;
    const POLLOUT: c_short = 0x0004;
    const POLLERR: c_short = 0x0008;
    const POLLHUP: c_short = 0x0010;
    const POLL_SLICE_MS: c_int = 250;

    #[repr(C)]
    struct PollFd {
        fd: c_int,
        events: c_short,
        revents: c_short,
    }

    unsafe extern "C" {
        #[link_name = "poll"]
        fn c_poll(fds: *mut PollFd, count: usize, timeout_ms: c_int) -> c_int;
        #[link_name = "read"]
        fn c_read(fd: c_int, buffer: *mut c_void, count: usize) -> isize;
        #[link_name = "write"]
        fn c_write(fd: c_int, buffer: *const c_void, count: usize) -> isize;
        #[link_name = "close"]
        fn c_close(fd: c_int) -> c_int;
    }

    #[derive(Debug)]
    struct UsbAbort {
        canceled: Arc<AtomicBool>,
    }

    impl SyncAbort for UsbAbort {
        fn abort(&self) {
            self.canceled.store(true, Ordering::SeqCst);
        }
    }

    pub(crate) struct AndroidUsbStream {
        fd: c_int,
        canceled: Arc<AtomicBool>,
    }

    unsafe impl Send for AndroidUsbStream {}

    impl fmt::Debug for AndroidUsbStream {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter
                .debug_struct("AndroidUsbStream")
                .finish_non_exhaustive()
        }
    }

    impl Drop for AndroidUsbStream {
        fn drop(&mut self) {
            if self.fd >= 0 {
                unsafe {
                    let _ = c_close(self.fd);
                }
                self.fd = -1;
            }
        }
    }

    impl SyncAbortSource for AndroidUsbStream {
        fn sync_abort_handle(&self) -> Result<Arc<dyn SyncAbort>, CommandError> {
            Ok(Arc::new(UsbAbort {
                canceled: Arc::clone(&self.canceled),
            }))
        }
    }

    impl SyncIo for AndroidUsbStream {
        fn prepare_for_sync(&self, _timeout: Duration) -> Result<(), CommandError> {
            Ok(())
        }
    }

    impl Read for AndroidUsbStream {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            if buffer.is_empty() {
                return Ok(0);
            }
            loop {
                wait_ready(self.fd, POLLIN, &self.canceled)?;
                let count = unsafe { c_read(self.fd, buffer.as_mut_ptr().cast(), buffer.len()) };
                if count >= 0 {
                    return Ok(usize::try_from(count).unwrap_or_default());
                }
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted
                    || error.kind() == io::ErrorKind::WouldBlock
                {
                    continue;
                }
                return Err(error);
            }
        }
    }

    impl Write for AndroidUsbStream {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            if buffer.is_empty() {
                return Ok(0);
            }
            loop {
                wait_ready(self.fd, POLLOUT, &self.canceled)?;
                let count = unsafe { c_write(self.fd, buffer.as_ptr().cast(), buffer.len()) };
                if count >= 0 {
                    return Ok(usize::try_from(count).unwrap_or_default());
                }
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted
                    || error.kind() == io::ErrorKind::WouldBlock
                {
                    continue;
                }
                return Err(error);
            }
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    pub(crate) fn from_accessory_fd(fd: i32) -> Result<AndroidUsbStream, CommandError> {
        if fd < 0 {
            return Err(CommandError::new(
                "device_sync_usb_open_failed",
                "Android returned an invalid USB accessory file descriptor.",
            ));
        }
        Ok(AndroidUsbStream {
            fd,
            canceled: Arc::new(AtomicBool::new(false)),
        })
    }

    fn wait_ready(fd: c_int, events: c_short, canceled: &AtomicBool) -> io::Result<()> {
        loop {
            if canceled.load(Ordering::SeqCst) {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "USB synchronization was canceled.",
                ));
            }
            let mut descriptor = PollFd {
                fd,
                events,
                revents: 0,
            };
            let status = unsafe { c_poll(&mut descriptor, 1, POLL_SLICE_MS) };
            if status == 0 {
                continue;
            }
            if status < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error);
            }
            if descriptor.revents & (POLLERR | POLLHUP) != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "The USB cable was disconnected during synchronization.",
                ));
            }
            if descriptor.revents & events != 0 {
                return Ok(());
            }
        }
    }
}
