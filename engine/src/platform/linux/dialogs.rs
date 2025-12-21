//! Linux native file dialogs using rfd
//!
//! Provides native file open/save dialogs that work on both GTK and Qt desktops.

use rfd::{FileDialog, MessageDialog, MessageButtons, MessageDialogResult};
use std::path::PathBuf;

/// Message dialog severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageLevel {
    Info,
    Warning,
    Error,
}

/// Open a file dialog to select a single file
///
/// # Arguments
/// * `title` - Dialog title
/// * `filters` - List of (description, extensions) pairs, e.g., [("Images", &["png", "jpg"])]
/// * `default_path` - Optional starting directory
pub fn open_file_dialog(
    title: &str,
    filters: &[(&str, &[&str])],
    default_path: Option<&str>,
) -> Option<PathBuf> {
    let mut dialog = FileDialog::new().set_title(title);

    for (name, exts) in filters {
        dialog = dialog.add_filter(*name, exts);
    }

    if let Some(path) = default_path {
        dialog = dialog.set_directory(path);
    }

    dialog.pick_file()
}

/// Open a file dialog to select multiple files
pub fn open_files_dialog(
    title: &str,
    filters: &[(&str, &[&str])],
    default_path: Option<&str>,
) -> Option<Vec<PathBuf>> {
    let mut dialog = FileDialog::new().set_title(title);

    for (name, exts) in filters {
        dialog = dialog.add_filter(*name, exts);
    }

    if let Some(path) = default_path {
        dialog = dialog.set_directory(path);
    }

    dialog.pick_files()
}

/// Open a save file dialog
///
/// # Arguments
/// * `title` - Dialog title
/// * `default_name` - Default file name
/// * `filters` - List of (description, extensions) pairs
/// * `default_path` - Optional starting directory
pub fn save_file_dialog(
    title: &str,
    default_name: &str,
    filters: &[(&str, &[&str])],
    default_path: Option<&str>,
) -> Option<PathBuf> {
    let mut dialog = FileDialog::new()
        .set_title(title)
        .set_file_name(default_name);

    for (name, exts) in filters {
        dialog = dialog.add_filter(*name, exts);
    }

    if let Some(path) = default_path {
        dialog = dialog.set_directory(path);
    }

    dialog.save_file()
}

/// Open a folder selection dialog
pub fn open_folder_dialog(title: &str, default_path: Option<&str>) -> Option<PathBuf> {
    let mut dialog = FileDialog::new().set_title(title);

    if let Some(path) = default_path {
        dialog = dialog.set_directory(path);
    }

    dialog.pick_folder()
}

/// Show a message dialog
///
/// # Arguments
/// * `title` - Dialog title
/// * `message` - Message text
/// * `level` - Severity level (Info, Warning, Error)
///
/// # Returns
/// `true` if OK was clicked, `false` otherwise
pub fn show_message_dialog(title: &str, message: &str, level: MessageLevel) -> bool {
    let rfd_level = match level {
        MessageLevel::Info => rfd::MessageLevel::Info,
        MessageLevel::Warning => rfd::MessageLevel::Warning,
        MessageLevel::Error => rfd::MessageLevel::Error,
    };

    let result = MessageDialog::new()
        .set_title(title)
        .set_description(message)
        .set_level(rfd_level)
        .set_buttons(MessageButtons::Ok)
        .show();

    matches!(result, MessageDialogResult::Ok)
}

/// Show a confirmation dialog with Yes/No buttons
///
/// # Returns
/// `true` if Yes was clicked, `false` if No was clicked
pub fn show_confirm_dialog(title: &str, message: &str) -> bool {
    let result = MessageDialog::new()
        .set_title(title)
        .set_description(message)
        .set_level(rfd::MessageLevel::Info)
        .set_buttons(MessageButtons::YesNo)
        .show();

    matches!(result, MessageDialogResult::Yes)
}

#[cfg(test)]
mod tests {
    // Dialog tests are interactive and require a display,
    // so they're not run in CI. Manual testing is required.
}
