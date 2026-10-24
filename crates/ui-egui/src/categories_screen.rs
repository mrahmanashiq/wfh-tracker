use egui::{ComboBox, TextEdit, Ui};
use wt_core::repo::RepoResult;
use wt_core::{Category, CategoryKind, CategoryRepo, Id, NewCategory, Rgb};

use crate::theme::{size, space};
use crate::widgets::{
    DeleteClick, Icon, card, color_dot, column, delete_buttons, empty_state, icon_button, list_row,
    message, pill, screen_title, text_button, truncated,
};

/// Add, edit and delete categories. Deleting one still used by tasks is refused.
pub struct CategoriesScreen {
    new_name: String,
    new_kind: CategoryKind,
    new_color: [u8; 3],
    /// Category being edited, with the edited name and colour.
    editing: Option<(Id, String, [u8; 3])>,
    confirm_delete: Option<Id>,
    message: Option<String>,
}

impl Default for CategoriesScreen {
    fn default() -> Self {
        Self {
            new_name: String::new(),
            new_kind: CategoryKind::Personal,
            new_color: [0x14, 0xb8, 0xa6],
            editing: None,
            confirm_delete: None,
            message: None,
        }
    }
}

enum Action {
    Add,
    OpenEdit(Id, String, [u8; 3]),
    Save,
    Delete(Id),
    ArmDelete(Option<Id>),
    Close,
}

impl CategoriesScreen {
    pub fn ui(&mut self, ui: &mut Ui, categories: &dyn CategoryRepo) {
        let p = crate::theme::palette(ui);
        screen_title(ui, Icon::Tag, p.accent, "Categories");

        let cats = match categories.list() {
            Ok(cats) => cats,
            Err(e) => {
                message(ui, &Some(format!("Could not load categories: {e}")));
                return;
            }
        };

        let mut action = None;
        ui.horizontal(|ui| {
            ui.add(TextEdit::singleline(&mut self.new_name).hint_text("Name"))
                .accessible_name("Category name");
            ComboBox::from_id_salt("new_category_kind")
                .selected_text(kind_label(self.new_kind))
                .show_ui(ui, |ui| {
                    for kind in CategoryKind::ALL {
                        ui.selectable_value(&mut self.new_kind, kind, kind_label(kind));
                    }
                })
                .response
                .accessible_name("Kind");
            ui.color_edit_button_srgb(&mut self.new_color)
                .accessible_name("Colour");
            if pill(ui, "Add category", p.accent, egui::Color32::WHITE).clicked() {
                action = Some(Action::Add);
            }
        });
        message(ui, &self.message);
        ui.add_space(space::MD);

        card(ui, |ui| {
            if cats.is_empty() {
                empty_state(
                    ui,
                    Icon::Tag,
                    "No categories yet.",
                    Some("Add one above to sort your tasks and timers."),
                );
            }
            for cat in &cats {
                self.category_row(ui, cat, &mut action);
            }
        });

        if let Some(action) = action {
            self.apply(action, categories);
        }
    }

    fn category_row(&mut self, ui: &mut Ui, cat: &Category, action: &mut Option<Action>) {
        let p = crate::theme::palette(ui);
        let id = cat.meta.id;
        list_row(ui, id, |ui| {
            color_dot(ui, cat.color);
            if let Some((_, name, color)) = self.editing.as_mut().filter(|e| e.0 == id) {
                ui.add(TextEdit::singleline(name).desired_width(size::LABEL_COLUMN))
                    .accessible_name("Edit category name");
                ui.color_edit_button_srgb(color)
                    .accessible_name("Edit colour");
                if text_button(ui, "Save", true).clicked() {
                    *action = Some(Action::Save);
                }
                if text_button(ui, "Cancel", false).clicked() {
                    *action = Some(Action::Close);
                }
                return;
            }
            column(ui, size::LABEL_COLUMN, |ui| truncated(ui, &cat.name));
            ui.weak(kind_label(cat.kind));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                match delete_buttons(ui, self.confirm_delete == Some(id)) {
                    Some(DeleteClick::Arm) => *action = Some(Action::ArmDelete(Some(id))),
                    Some(DeleteClick::Confirm) => *action = Some(Action::Delete(id)),
                    Some(DeleteClick::Cancel) => *action = Some(Action::ArmDelete(None)),
                    None => {}
                }
                if icon_button(ui, Icon::Pencil, "Edit", p.secondary).clicked() {
                    let Rgb(r, g, b) = cat.color;
                    *action = Some(Action::OpenEdit(id, cat.name.clone(), [r, g, b]));
                }
            });
        });
    }

    fn apply(&mut self, action: Action, categories: &dyn CategoryRepo) {
        let result: RepoResult<()> = match action {
            Action::Add => {
                let name = self.new_name.trim();
                if name.is_empty() {
                    self.message = Some("a category needs a name".to_owned());
                    return;
                }
                let [r, g, b] = self.new_color;
                categories
                    .create(NewCategory::new(name, self.new_kind, Rgb(r, g, b)))
                    .map(|_| self.new_name.clear())
            }
            Action::OpenEdit(id, name, color) => {
                self.editing = Some((id, name, color));
                Ok(())
            }
            Action::Save => match self.editing.take() {
                Some((id, name, [r, g, b])) if !name.trim().is_empty() => {
                    categories.update(id, name.trim(), Rgb(r, g, b)).map(|_| ())
                }
                Some(_) => {
                    self.message = Some("a category needs a name".to_owned());
                    return;
                }
                None => Ok(()),
            },
            Action::Delete(id) => {
                self.confirm_delete = None;
                categories.delete(id)
            }
            Action::ArmDelete(id) => {
                self.confirm_delete = id;
                Ok(())
            }
            Action::Close => {
                self.editing = None;
                Ok(())
            }
        };
        self.message = result.err().map(|e| e.to_string());
    }
}

fn kind_label(kind: CategoryKind) -> &'static str {
    match kind {
        CategoryKind::Office => "Office",
        CategoryKind::Meeting => "Meeting",
        CategoryKind::Learning => "Learning",
        CategoryKind::Personal => "Personal",
        CategoryKind::Break => "Break",
        CategoryKind::Custom => "Custom",
    }
}
