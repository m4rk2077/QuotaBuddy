#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackdropMode {
    DesktopAcrylic,
    #[default]
    Solid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackdropCapability {
    pub windows_build: u32,
    pub high_contrast: bool,
    pub transparency_enabled: bool,
}

pub fn select_backdrop(capability: BackdropCapability) -> BackdropMode {
    if capability.windows_build >= 22_621
        && !capability.high_contrast
        && capability.transparency_enabled
    {
        BackdropMode::DesktopAcrylic
    } else {
        BackdropMode::Solid
    }
}

#[cfg(windows)]
pub fn apply_to_window(window: &tauri::WebviewWindow) -> BackdropMode {
    use std::ffi::c_void;
    use windows::{
        Win32::Graphics::Dwm::{
            DwmSetWindowAttribute, DWMSBT_NONE, DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE,
        },
        UI::ViewManagement::{AccessibilitySettings, UISettings},
    };

    let capability = BackdropCapability {
        windows_build: windows_version::OsVersion::current().build,
        high_contrast: AccessibilitySettings::new()
            .and_then(|settings| settings.HighContrast())
            .unwrap_or(true),
        transparency_enabled: UISettings::new()
            .and_then(|settings| settings.AdvancedEffectsEnabled())
            .unwrap_or(false),
    };
    let requested = select_backdrop(capability);
    let Ok(hwnd) = window.hwnd() else {
        return BackdropMode::Solid;
    };
    let value = match requested {
        BackdropMode::DesktopAcrylic => DWMSBT_TRANSIENTWINDOW,
        BackdropMode::Solid => DWMSBT_NONE,
    };
    let set_backdrop = |value| unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            &value as *const _ as *const c_void,
            std::mem::size_of_val(&value) as u32,
        )
    };
    let applied = set_backdrop(value);

    if requested == BackdropMode::DesktopAcrylic && applied.is_ok() {
        BackdropMode::DesktopAcrylic
    } else {
        if requested == BackdropMode::DesktopAcrylic {
            let _ = set_backdrop(DWMSBT_NONE);
        }
        BackdropMode::Solid
    }
}

#[cfg(not(windows))]
pub fn apply_to_window(_window: &tauri::WebviewWindow) -> BackdropMode {
    BackdropMode::Solid
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enables_desktop_acrylic_on_supported_windows_11() {
        assert_eq!(
            select_backdrop(BackdropCapability {
                windows_build: 22_621,
                high_contrast: false,
                transparency_enabled: true,
            }),
            BackdropMode::DesktopAcrylic
        );
    }

    #[test]
    fn falls_back_before_windows_11_22h2() {
        assert_eq!(
            select_backdrop(BackdropCapability {
                windows_build: 22_620,
                high_contrast: false,
                transparency_enabled: true,
            }),
            BackdropMode::Solid
        );
    }

    #[test]
    fn high_contrast_forces_solid_fallback() {
        assert_eq!(
            select_backdrop(BackdropCapability {
                windows_build: 26_200,
                high_contrast: true,
                transparency_enabled: true,
            }),
            BackdropMode::Solid
        );
    }

    #[test]
    fn disabled_transparency_forces_solid_fallback() {
        assert_eq!(
            select_backdrop(BackdropCapability {
                windows_build: 26_200,
                high_contrast: false,
                transparency_enabled: false,
            }),
            BackdropMode::Solid
        );
    }
}
