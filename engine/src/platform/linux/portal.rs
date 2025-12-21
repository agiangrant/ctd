//! XDG Portal integration via D-Bus
//!
//! Provides access to freedesktop.org portal APIs for:
//! - Dark mode detection
//! - Accent color
//! - Other desktop settings

use std::sync::OnceLock;
use tokio::runtime::Runtime;
use zbus::{Connection, Result as ZbusResult};

/// Get or create the async runtime for D-Bus operations
fn get_runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime for D-Bus")
    })
}

/// Check if the system is in dark mode
///
/// Uses the freedesktop.org Settings portal to detect the color scheme preference.
/// Returns `true` if dark mode is preferred, `false` otherwise.
pub fn is_dark_mode() -> bool {
    get_runtime().block_on(async {
        is_dark_mode_async().await.unwrap_or(false)
    })
}

/// Async implementation of dark mode detection
async fn is_dark_mode_async() -> ZbusResult<bool> {
    let connection = Connection::session().await?;

    // Call org.freedesktop.portal.Settings.Read
    let reply: zbus::Message = connection
        .call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.portal.Settings"),
            "Read",
            &("org.freedesktop.appearance", "color-scheme"),
        )
        .await?;

    // The reply is a variant containing a variant containing a u32
    // 0 = no preference, 1 = prefer dark, 2 = prefer light
    let body = reply.body();
    if let Ok(value) = body.deserialize::<zbus::zvariant::OwnedValue>() {
        // Try to extract the nested value
        // The portal returns v(v(u)) - variant of variant of u32
        if let Ok(inner) = value.try_to_owned() {
            if let Ok(scheme) = inner.try_into() {
                let scheme: u32 = scheme;
                return Ok(scheme == 1); // 1 = prefer dark
            }
        }
    }

    Ok(false)
}

/// Get the system accent color
///
/// Uses the freedesktop.org Settings portal to get the accent color.
/// Returns RGBA as (r, g, b, a) in 0-255 range, or None if not available.
pub fn get_accent_color() -> Option<(u8, u8, u8, u8)> {
    get_runtime().block_on(async {
        get_accent_color_async().await.ok().flatten()
    })
}

/// Async implementation of accent color retrieval
async fn get_accent_color_async() -> ZbusResult<Option<(u8, u8, u8, u8)>> {
    let connection = Connection::session().await?;

    // Call org.freedesktop.portal.Settings.Read
    let reply: zbus::Message = connection
        .call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.portal.Settings"),
            "Read",
            &("org.freedesktop.appearance", "accent-color"),
        )
        .await?;

    // The accent color is returned as (ddd) - three doubles for RGB
    let body = reply.body();
    if let Ok(value) = body.deserialize::<zbus::zvariant::OwnedValue>() {
        // Try to extract as tuple of 3 floats
        if let Ok((r, g, b)) = value.try_into() {
            let (r, g, b): (f64, f64, f64) = (r, g, b);
            return Ok(Some((
                (r * 255.0) as u8,
                (g * 255.0) as u8,
                (b * 255.0) as u8,
                255,
            )));
        }
    }

    Ok(None)
}

/// Get the system contrast preference
///
/// Returns the contrast preference:
/// - 0 = no preference
/// - 1 = less contrast
/// - 2 = more contrast
pub fn get_contrast_preference() -> u32 {
    get_runtime().block_on(async {
        get_contrast_async().await.unwrap_or(0)
    })
}

async fn get_contrast_async() -> ZbusResult<u32> {
    let connection = Connection::session().await?;

    let reply: zbus::Message = connection
        .call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.portal.Settings"),
            "Read",
            &("org.freedesktop.appearance", "contrast"),
        )
        .await?;

    let body = reply.body();
    if let Ok(value) = body.deserialize::<zbus::zvariant::OwnedValue>() {
        if let Ok(contrast) = value.try_into() {
            return Ok(contrast);
        }
    }

    Ok(0)
}

/// Check if natural scrolling is enabled
///
/// Checks GNOME/KDE settings for mouse (primary desktop input).
/// Returns `true` if natural scrolling is enabled.
/// Defaults to `false` (traditional scrolling) if detection fails.
///
/// Note: Not cached - allows runtime changes to take effect immediately.
pub fn is_natural_scrolling() -> bool {
    // Try GNOME settings via gsettings command
    // Check mouse first (primary desktop input device)
    if let Ok(output) = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.peripherals.mouse", "natural-scroll"])
        .output()
    {
        if output.status.success() {
            let value = String::from_utf8_lossy(&output.stdout);
            return value.trim() == "true";
        }
    }

    // Fall back to touchpad settings
    if let Ok(output) = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.peripherals.touchpad", "natural-scroll"])
        .output()
    {
        if output.status.success() {
            let value = String::from_utf8_lossy(&output.stdout);
            return value.trim() == "true";
        }
    }

    // Try KDE settings (Plasma uses different config)
    if let Ok(output) = std::process::Command::new("kreadconfig5")
        .args(["--file", "kcminputrc", "--group", "Libinput", "--key", "NaturalScroll"])
        .output()
    {
        if output.status.success() {
            let value = String::from_utf8_lossy(&output.stdout);
            return value.trim() == "true";
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dark_mode_detection() {
        // This test may fail in CI without a D-Bus session
        // Just verify it doesn't panic
        let _ = is_dark_mode();
    }

    #[test]
    fn test_accent_color() {
        // This test may fail in CI without a D-Bus session
        // Just verify it doesn't panic
        let _ = get_accent_color();
    }
}
