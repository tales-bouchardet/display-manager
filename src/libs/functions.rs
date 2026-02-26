use windows::Win32::Foundation::{
    LPARAM,
    RECT,
    GetLastError,
};
use windows::core::{
    Result,
    BOOL,
    HRESULT,
    PCWSTR
};
use windows::Win32::Graphics::Gdi::{
    ChangeDisplaySettingsExW,
    EnumDisplaySettingsExW,
    DEVMODEW,
    CDS_UPDATEREGISTRY,
    DM_DISPLAYFREQUENCY,
    DM_PELSHEIGHT,
    DM_PELSWIDTH,
    DM_POSITION,
    ENUM_CURRENT_SETTINGS,
    ENUM_DISPLAY_SETTINGS_MODE,
    ENUM_DISPLAY_SETTINGS_FLAGS,
    DISP_CHANGE_SUCCESSFUL,
    EnumDisplayMonitors, 
    GetMonitorInfoW, 
    HMONITOR, 
    MONITORINFOEXW, 
    HDC,
};
use windows::Win32::Devices::Display::{
    GetPhysicalMonitorsFromHMONITOR,
    DestroyPhysicalMonitors,
    SetVCPFeature,
    PHYSICAL_MONITOR,
    GetDisplayConfigBufferSizes,
    QueryDisplayConfig,
    SetDisplayConfig,
    DISPLAYCONFIG_MODE_INFO,
    DISPLAYCONFIG_PATH_INFO,
    DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE,
    QDC_ONLY_ACTIVE_PATHS,
    SDC_ALLOW_CHANGES,
    SDC_APPLY,
    SDC_SAVE_TO_DATABASE,
    SDC_USE_SUPPLIED_DISPLAY_CONFIG,
};

#[derive(Debug)]
pub struct DisplaySummary {
    pub index: u32,
    pub name: String,
}

#[derive(Debug)]
pub struct DisplayInfo {
    pub index: u32,
    pub name: String,
    pub position: RECT,
    pub resolution: Resolution,
    pub is_primary: bool,
    pub hmonitor: HMONITOR,
    pub supported_resolutions: Vec<Resolutions>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Resolution {
    pub w: i32,
    pub h: i32,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Resolutions {
    pub sw: u32,
    pub sh: u32,
}

pub fn find_properties(index: u32) -> Result<DisplayInfo> {
    let mut displays: Vec<DisplayInfo> = Vec::new();

    unsafe extern "system" fn monitor_enum_proc(
        hmonitor: HMONITOR,
        _hdc: HDC,
        _lprc: *mut RECT,
        dw_data: LPARAM,
    ) -> BOOL {
        let displays = unsafe { &mut *(dw_data.0 as *mut Vec<DisplayInfo>) };

        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;

        if unsafe { GetMonitorInfoW(hmonitor, &mut info.monitorInfo) }.as_bool() {
            let name = String::from_utf16_lossy(&info.szDevice)
                .trim_end_matches('\0')
                .to_string();

            let rc = info.monitorInfo.rcMonitor;
            let width = rc.right - rc.left;
            let height = rc.bottom - rc.top;
            let is_primary = (info.monitorInfo.dwFlags & 1) != 0;
            let index = displays.len() as u32;

            let mut supported_resolutions: std::collections::HashSet<Resolutions> = std::collections::HashSet::new();
            let device_wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
            let device_name = PCWSTR(device_wide.as_ptr());

            let mut i = 0u32;
            loop {
                let mut devmode = DEVMODEW::default();
                devmode.dmSize = std::mem::size_of::<DEVMODEW>() as u16;

                if unsafe {
                    EnumDisplaySettingsExW(
                        device_name,
                        ENUM_DISPLAY_SETTINGS_MODE(i),
                        &mut devmode,
                        ENUM_DISPLAY_SETTINGS_FLAGS(0),
                    )
                }
                .as_bool()
                {
                    supported_resolutions.insert(Resolutions {
                        sw: devmode.dmPelsWidth,
                        sh: devmode.dmPelsHeight,
                    });
                    i += 1;
                } else {
                    break;
                }
            }

            let mut sorted: Vec<Resolutions> = supported_resolutions.into_iter().collect();
            sorted.sort_by(|a, b| (b.sh).cmp(&(a.sh)));

            displays.push(DisplayInfo {
                index,
                name,
                position: rc,
                resolution: Resolution { w: width, h: height },
                is_primary,
                hmonitor,
                supported_resolutions: sorted,
            });
        }
        BOOL(1)
    }

    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(monitor_enum_proc),
            LPARAM(&mut displays as *mut _ as isize),
        );
    }

    displays
        .into_iter()
        .find(|d| d.index == index)
        .ok_or_else(|| windows::core::Error::new(
            HRESULT::from_win32(0x57),
            format!("Monitor com index {} não encontrado", index),
        ))
}

pub fn list_displays() -> Result<Vec<DisplaySummary>> {
    let mut displays: Vec<DisplaySummary> = Vec::new();

    unsafe extern "system" fn monitor_enum_proc(
        hmonitor: HMONITOR,
        _hdc: HDC,
        _lprc: *mut RECT,
        dw_data: LPARAM,
    ) -> BOOL {
        let displays = unsafe { &mut *(dw_data.0 as *mut Vec<DisplaySummary>) };

        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;

        if unsafe { GetMonitorInfoW(hmonitor, &mut info.monitorInfo) }.as_bool() {
            let name = String::from_utf16_lossy(&info.szDevice)
                .trim_end_matches('\0')
                .to_string();
            let index = displays.len() as u32;

            displays.push(DisplaySummary { index, name });
        }
        BOOL(1)
    }

    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(monitor_enum_proc),
            LPARAM(&mut displays as *mut _ as isize),
        );
    }

    Ok(displays)
}

pub fn set_resolution(index: u32, w: u32, h: u32) -> Result<()> {
    let display = find_properties(index)?;

    let device_wide: Vec<u16> = display.name
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let device_name = PCWSTR(device_wide.as_ptr());

    unsafe {
        let mut min_freq = u32::MAX;
        let mut best_mode: Option<DEVMODEW> = None;

        let mut i = 0u32;
        loop {
            let mut devmode = DEVMODEW::default();
            devmode.dmSize = std::mem::size_of::<DEVMODEW>() as u16;

            if !EnumDisplaySettingsExW(
                device_name,
                ENUM_DISPLAY_SETTINGS_MODE(i),
                &mut devmode,
                ENUM_DISPLAY_SETTINGS_FLAGS(0),
            ).as_bool() {
                break;
            }

            if devmode.dmPelsWidth == w
                && devmode.dmPelsHeight == h
                && devmode.dmDisplayFrequency > 0
                && devmode.dmDisplayFrequency < min_freq
            {
                min_freq = devmode.dmDisplayFrequency;
                best_mode = Some(devmode);
            }

            i += 1;
        }

        let mut mode = best_mode.ok_or_else(|| {
            windows::core::Error::new(
                HRESULT::from_win32(0x80070057),
                format!("Resolução {}x{} não suportada no monitor '{}'", w, h, display.name),
            )
        })?;

        mode.dmFields = DM_PELSWIDTH | DM_PELSHEIGHT | DM_DISPLAYFREQUENCY;

        let result = ChangeDisplaySettingsExW(
            device_name,
            Some(&mode as *const DEVMODEW),
            None,
            CDS_UPDATEREGISTRY,
            None,
        );

        if result != DISP_CHANGE_SUCCESSFUL {
            return Err(windows::core::Error::from_hresult(
                HRESULT::from_win32(result.0 as u32)
            ));
        }

        Ok(())
    }
}

pub fn set_primary_display(index: u32) -> Result<()> {
    let display = find_properties(index)?;

    if display.is_primary {
        return Ok(());
    }

    let offset_x = display.position.left;
    let offset_y = display.position.top;

    unsafe {
        let mut num_paths: u32 = 0;
        let mut num_modes: u32 = 0;

        GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut num_paths, &mut num_modes)
            .ok()
            .map_err(|e| windows::core::Error::new(HRESULT::from_win32(0x80070057), format!("{:?}", e)))?;

        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); num_paths as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); num_modes as usize];

        QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut num_paths,
            paths.as_mut_ptr(),
            &mut num_modes,
            modes.as_mut_ptr(),
            None,
        )
        .ok()
        .map_err(|e| windows::core::Error::new(HRESULT::from_win32(0x80070057), format!("{:?}", e)))?;

        paths.truncate(num_paths as usize);
        modes.truncate(num_modes as usize);

        for mode in modes.iter_mut() {
            if mode.infoType == DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE {
                let source = &mut mode.Anonymous.sourceMode;
                source.position.x -= offset_x;
                source.position.y -= offset_y;
            }
        }

        let result = SetDisplayConfig(
            Some(&paths),
            Some(&modes),
            SDC_APPLY | SDC_USE_SUPPLIED_DISPLAY_CONFIG | SDC_ALLOW_CHANGES | SDC_SAVE_TO_DATABASE,
        );

        if result != 0 {
            return Err(windows::core::Error::new(
                HRESULT::from_win32(result as u32),
                format!("SetDisplayConfig falhou com código {}", result),
            ));
        }
    }

    Ok(())
}

pub fn auto_adjust(index: u32) -> Result<()> {
    let display = find_properties(index)?;

    unsafe {
        let mut physical_array = vec![PHYSICAL_MONITOR::default(); 1];

        GetPhysicalMonitorsFromHMONITOR(display.hmonitor, &mut physical_array)?;

        let h_physical = physical_array[0].hPhysicalMonitor;

        if SetVCPFeature(h_physical, 0x1E, 1) == 0 {
            let _ = DestroyPhysicalMonitors(&physical_array);
            let last_error = GetLastError();
            return Err(windows::core::Error::from_hresult(
                HRESULT::from_win32(last_error.0)
            ));
        }

        let _ = DestroyPhysicalMonitors(&physical_array);

        std::thread::sleep(std::time::Duration::from_millis(3000));

        Ok(())
    }
}

pub fn move_display(index: u32, x: i32, y: i32) -> Result<()> {
    let display = find_properties(index)?;

    let device_wide: Vec<u16> = display.name
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let device_name = PCWSTR(device_wide.as_ptr());

    unsafe {
        let mut devmode = DEVMODEW::default();
        devmode.dmSize = std::mem::size_of::<DEVMODEW>() as u16;

        if !EnumDisplaySettingsExW(
            device_name,
            ENUM_DISPLAY_SETTINGS_MODE(ENUM_CURRENT_SETTINGS.0 as u32),
            &mut devmode,
            ENUM_DISPLAY_SETTINGS_FLAGS(0),
        ).as_bool() {
            return Err(windows::core::Error::new(
                HRESULT::from_win32(0x80070057),
                format!("Não foi possível obter configuração atual do monitor '{}'", display.name),
            ));
        }

        devmode.Anonymous1.Anonymous2.dmPosition.x = x;
        devmode.Anonymous1.Anonymous2.dmPosition.y = y;
        devmode.dmFields = DM_POSITION;

        let result = ChangeDisplaySettingsExW(
            device_name,
            Some(&devmode as *const DEVMODEW),
            None,
            CDS_UPDATEREGISTRY,
            None,
        );

        if result != DISP_CHANGE_SUCCESSFUL {
            return Err(windows::core::Error::from_hresult(
                HRESULT::from_win32(result.0 as u32)
            ));
        }

        Ok(())
    }
}

pub fn display_brightness(index: u32, percent: u32) -> Result<()> {
    let display = find_properties(index)?;

    unsafe {
        let mut physical_array = vec![PHYSICAL_MONITOR::default(); 1];

        GetPhysicalMonitorsFromHMONITOR(display.hmonitor, &mut physical_array)?;

        let h_physical = physical_array[0].hPhysicalMonitor;

        if SetVCPFeature(h_physical, 0x10, percent) == 0 {
            let _ = DestroyPhysicalMonitors(&physical_array);
            let last_error = GetLastError();
            return Err(windows::core::Error::from_hresult(
                HRESULT::from_win32(last_error.0)
            ));
        }

        let _ = DestroyPhysicalMonitors(&physical_array);

        Ok(())
    }
}

pub fn reset_monitor(index: u32) -> Result<()> {
    let display = find_properties(index)?;

    unsafe {
        let mut physical_array = vec![PHYSICAL_MONITOR::default(); 1];

        GetPhysicalMonitorsFromHMONITOR(display.hmonitor, &mut physical_array)?;

        let h_physical = physical_array[0].hPhysicalMonitor;

        let reset_codes = [0x08, 0x04, 0x06, 0x05];

        for code in reset_codes {
            if SetVCPFeature(h_physical, code, 1) == 0 {
                let _ = DestroyPhysicalMonitors(&physical_array);
                let last_error = GetLastError();
                return Err(windows::core::Error::from_hresult(
                    HRESULT::from_win32(last_error.0)
                ));
            }
        }

        let _ = DestroyPhysicalMonitors(&physical_array);

        Ok(())
    }
}

pub fn verify_vcp(index: u32) -> Result<(bool, u32)> {
    use windows::Win32::Devices::Display::{
        GetVCPFeatureAndVCPFeatureReply,
        MC_MOMENTARY,
    };

    let display = find_properties(index)?;

    unsafe {
        let mut physical_array = vec![PHYSICAL_MONITOR::default(); 1];

        GetPhysicalMonitorsFromHMONITOR(display.hmonitor, &mut physical_array)?;

        let h_physical = physical_array[0].hPhysicalMonitor;

        let mut current_value: u32 = 0;
        let mut max_value: u32 = 0;
        let mut vcp_type = MC_MOMENTARY;

        let supported = GetVCPFeatureAndVCPFeatureReply(
            h_physical,
            0x10,
            Some(&mut vcp_type),
            &mut current_value,
            Some(&mut max_value),
        ) != 0;

        let _ = DestroyPhysicalMonitors(&physical_array);

        Ok((supported, current_value))
    }
}
