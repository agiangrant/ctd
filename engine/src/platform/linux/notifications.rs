//! Linux desktop notifications using notify-rust
//!
//! Provides desktop notification functionality via D-Bus.

use notify_rust::{Notification, Timeout, Urgency};

/// Show a simple notification
///
/// # Arguments
/// * `title` - Notification title (summary)
/// * `body` - Notification body text
pub fn show_notification(title: &str, body: &str) {
    let _ = Notification::new()
        .summary(title)
        .body(body)
        .timeout(Timeout::Default)
        .show();
}

/// Show a notification with an icon
///
/// # Arguments
/// * `title` - Notification title
/// * `body` - Notification body text
/// * `icon` - Icon name (from icon theme) or path to icon file
pub fn show_notification_with_icon(title: &str, body: &str, icon: &str) {
    let _ = Notification::new()
        .summary(title)
        .body(body)
        .icon(icon)
        .timeout(Timeout::Default)
        .show();
}

/// Notification urgency level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationUrgency {
    Low,
    Normal,
    Critical,
}

/// Show a notification with full options
///
/// # Arguments
/// * `title` - Notification title
/// * `body` - Notification body text
/// * `icon` - Optional icon name or path
/// * `urgency` - Notification urgency level
/// * `timeout_ms` - Timeout in milliseconds (0 = never expire)
/// * `app_name` - Application name
pub fn show_notification_full(
    title: &str,
    body: &str,
    icon: Option<&str>,
    urgency: NotificationUrgency,
    timeout_ms: u32,
    app_name: &str,
) {
    let rust_urgency = match urgency {
        NotificationUrgency::Low => Urgency::Low,
        NotificationUrgency::Normal => Urgency::Normal,
        NotificationUrgency::Critical => Urgency::Critical,
    };

    let timeout = if timeout_ms == 0 {
        Timeout::Never
    } else {
        Timeout::Milliseconds(timeout_ms)
    };

    let mut notification = Notification::new();
    notification
        .summary(title)
        .body(body)
        .appname(app_name)
        .urgency(rust_urgency)
        .timeout(timeout);

    if let Some(icon_name) = icon {
        notification.icon(icon_name);
    }

    let _ = notification.show();
}

/// Show a notification with action buttons
///
/// Returns the action ID that was clicked, or None if the notification was dismissed.
///
/// # Arguments
/// * `title` - Notification title
/// * `body` - Notification body text
/// * `actions` - List of (action_id, action_label) pairs
pub fn show_notification_with_actions(
    title: &str,
    body: &str,
    actions: &[(&str, &str)],
) -> Option<String> {
    use std::sync::{Arc, Mutex};

    let mut notification = Notification::new();
    notification.summary(title).body(body);

    for (id, label) in actions {
        notification.action(id, label);
    }

    match notification.show() {
        Ok(handle) => {
            // Capture the clicked action
            let clicked_action = Arc::new(Mutex::new(None::<String>));
            let clicked_clone = clicked_action.clone();

            handle.wait_for_action(move |action| {
                if action != "__closed" {
                    if let Ok(mut guard) = clicked_clone.lock() {
                        *guard = Some(action.to_string());
                    }
                }
            });

            // Extract the result before returning to satisfy borrow checker
            let result = match clicked_action.lock() {
                Ok(guard) => guard.clone(),
                Err(_) => None,
            };
            result
        }
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    // Notification tests require a running D-Bus session,
    // so they're not run in CI. Manual testing is required.
}
