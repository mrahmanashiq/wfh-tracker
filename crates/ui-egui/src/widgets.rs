//! Small pieces shared by the screens.

use egui::{Color32, Sense, Ui, vec2};
use wt_core::Rgb;

pub fn color32(c: Rgb) -> Color32 {
    Color32::from_rgb(c.0, c.1, c.2)
}

/// A small filled circle in a category's colour.
pub fn color_dot(ui: &mut Ui, color: Rgb) {
    let (rect, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
    ui.painter()
        .circle_filled(rect.center(), 5.0, color32(color));
}

/// Shows the result of the last action: errors in the theme's error colour.
pub fn message(ui: &mut Ui, message: &Option<String>) {
    if let Some(text) = message {
        ui.colored_label(ui.visuals().error_fg_color, text);
    }
}

pub enum DeleteClick {
    Arm,
    Confirm,
    Cancel,
}

/// Two-step delete: "Delete" arms it, then "Confirm delete" or "Cancel".
pub fn delete_buttons(ui: &mut Ui, armed: bool) -> Option<DeleteClick> {
    if !armed {
        return ui.button("Delete").clicked().then_some(DeleteClick::Arm);
    }
    if ui.button("Confirm delete").clicked() {
        Some(DeleteClick::Confirm)
    } else if ui.button("Cancel").clicked() {
        Some(DeleteClick::Cancel)
    } else {
        None
    }
}
