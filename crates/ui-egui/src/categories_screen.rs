use egui::{ComboBox, TextEdit, Ui};
use wt_core::repo::RepoResult;
use wt_core::{CategoryKind, CategoryRepo, Id, NewCategory, Rgb};

use crate::widgets::{DeleteClick, color_dot, delete_buttons, message};

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
        ui.heading("Categories");
        ui.add_space(8.0);

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
            if ui.button("Add category").clicked() {
                action = Some(Action::Add);
            }
        });
        message(ui, &self.message);
        ui.separator();

        for cat in &cats {
            let id = cat.meta.id;
            ui.horizontal(|ui| {
                color_dot(ui, cat.color);
                if let Some((_, name, color)) = self.editing.as_mut().filter(|e| e.0 == id) {
                    ui.add(TextEdit::singleline(name).desired_width(180.0))
                        .accessible_name("Edit category name");
                    ui.color_edit_button_srgb(color)
                        .accessible_name("Edit colour");
                    if ui.button("Save").clicked() {
                        action = Some(Action::Save);
                    }
                    if ui.button("Cancel").clicked() {
                        action = Some(Action::Close);
                    }
                    return;
                }
                ui.label(&cat.name);
                ui.weak(kind_label(cat.kind));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    match delete_buttons(ui, self.confirm_delete == Some(id)) {
                        Some(DeleteClick::Arm) => action = Some(Action::ArmDelete(Some(id))),
                        Some(DeleteClick::Confirm) => action = Some(Action::Delete(id)),
                        Some(DeleteClick::Cancel) => action = Some(Action::ArmDelete(None)),
                        None => {}
                    }
                    if ui.button("Edit").clicked() {
                        let Rgb(r, g, b) = cat.color;
                        action = Some(Action::OpenEdit(id, cat.name.clone(), [r, g, b]));
                    }
                });
            });
        }

        if let Some(action) = action {
            self.apply(action, categories);
        }
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
