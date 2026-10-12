//! Choosing a file by walking through folders, in a window of the editor's own: it looks
//! and works the same wherever the editor runs, and needs nothing from the system.

use std::{
    fs,
    path::{Path, PathBuf},
};

use eframe::egui::{self, Align2, RichText};

use crate::style;

/// What a file is being chosen for.
#[derive(Clone, Copy, PartialEq)]
pub enum Purpose {
    OpenDeck,
    SaveDeck,
    Picture,
}

impl Purpose {
    fn title(self) -> &'static str {
        match self {
            Purpose::OpenDeck => "Open a deck",
            Purpose::SaveDeck => "Save the deck as",
            Purpose::Picture => "Choose a picture",
        }
    }

    fn confirm(self) -> &'static str {
        match self {
            Purpose::OpenDeck => "Open",
            Purpose::SaveDeck => "Save",
            Purpose::Picture => "Use this picture",
        }
    }

    /// Whether a file of this name is the kind being looked for.
    fn shows(self, name: &str) -> bool {
        let name = name.to_lowercase();
        let endings: &[&str] = match self {
            Purpose::OpenDeck | Purpose::SaveDeck => &[DECK_ENDING],
            Purpose::Picture => &[".png", ".jpg", ".jpeg", ".gif", ".bmp", ".webp"],
        };
        endings.iter().any(|ending| name.ends_with(ending))
    }
}

/// What a deck's file name ends with.
pub const DECK_ENDING: &str = ".suit.json";

struct Entry {
    name: String,
    folder: bool,
}

pub struct Browser {
    purpose: Purpose,
    folder: PathBuf,
    /// The file's name: picked from the list, or typed.
    name: String,
    /// The folder as text, for typing or pasting a path.
    typed: String,
    problem: Option<String>,
    entries: Vec<Entry>,
    /// The file that saving would replace, once that has been pointed out.
    replacing: Option<PathBuf>,
}

/// What became of the question.
pub enum Answer {
    /// Still open.
    Pending,
    Cancelled,
    Chosen(PathBuf),
}

impl Browser {
    /// Starts at `start`: a folder, or a file whose folder is shown and whose name is
    /// filled in.
    pub fn new(purpose: Purpose, start: &Path) -> Browser {
        let (folder, name) = match start.is_dir() {
            true => (start.to_path_buf(), String::new()),
            false => {
                let name = start.file_name().unwrap_or_default().to_string_lossy();
                let folder = start.parent().filter(|parent| parent.is_dir());
                let fallback = || std::env::current_dir().unwrap_or_default();
                (
                    folder.map_or_else(fallback, Path::to_path_buf),
                    name.into_owned(),
                )
            }
        };
        let mut browser = Browser {
            purpose,
            folder: PathBuf::new(),
            name,
            typed: String::new(),
            problem: None,
            entries: Vec::new(),
            replacing: None,
        };
        browser.enter(folder);
        browser
    }

    /// Reports `problem` under the list, for a choice that did not work out.
    pub fn with_problem(mut self, problem: String) -> Browser {
        self.problem = Some(problem);
        self
    }

    fn enter(&mut self, folder: PathBuf) {
        let folder = suit_render::plain(folder.canonicalize().unwrap_or(folder));
        let listing = match fs::read_dir(&folder) {
            Ok(listing) => listing,
            Err(reason) => {
                self.problem = Some(format!("Cannot look into {}: {reason}", folder.display()));
                return;
            }
        };
        let mut entries: Vec<Entry> = listing
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                // Following links, so that a link to a folder can be entered.
                let folder = entry.path().is_dir();
                let wanted = !name.starts_with('.') && (folder || self.purpose.shows(&name));
                wanted.then_some(Entry { name, folder })
            })
            .collect();
        entries.sort_by_key(|entry| (!entry.folder, entry.name.to_lowercase()));
        self.entries = entries;
        self.typed = folder.display().to_string();
        self.folder = folder;
        self.problem = None;
        self.replacing = None;
    }

    /// The file the name stands for, with a deck's ending added if it was left off.
    fn file(&self) -> Option<PathBuf> {
        let name = self.name.trim();
        if name.is_empty() {
            return None;
        }
        let complete = match self.purpose {
            Purpose::SaveDeck if !name.to_lowercase().ends_with(DECK_ENDING) => {
                format!("{}{DECK_ENDING}", name.trim_end_matches(".json"))
            }
            _ => name.to_owned(),
        };
        // A whole path typed into the name is taken at its word.
        Some(self.folder.join(complete))
    }

    fn confirm(&mut self) -> Answer {
        let Some(file) = self.file() else {
            self.problem = Some(match self.purpose {
                Purpose::SaveDeck => "Give the file a name.".to_owned(),
                _ => "Pick a file from the list.".to_owned(),
            });
            return Answer::Pending;
        };
        if file.is_dir() {
            self.name.clear();
            self.enter(file);
            return Answer::Pending;
        }
        match self.purpose {
            Purpose::SaveDeck if file.exists() && self.replacing.as_ref() != Some(&file) => {
                let name = file.file_name().unwrap_or_default().to_string_lossy();
                self.problem = Some(format!(
                    "{name} is already there. Press Save again to replace it."
                ));
                self.replacing = Some(file);
                Answer::Pending
            }
            Purpose::OpenDeck | Purpose::Picture if !file.is_file() => {
                self.problem = Some(format!("There is no file {}.", file.display()));
                Answer::Pending
            }
            _ => Answer::Chosen(file),
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Answer {
        let mut answer = Answer::Pending;
        let mut confirmed = false;
        let mut go = None;
        egui::Window::new(self.purpose.title())
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_width(600.0);
                ui.horizontal(|ui| {
                    let parent = self.folder.parent().map(Path::to_path_buf);
                    if ui
                        .add_enabled(parent.is_some(), egui::Button::new("↑ Up"))
                        .on_hover_text("The folder this one is in")
                        .clicked()
                    {
                        go = parent;
                    }
                    if let Some(home) = home()
                        && ui.button("Home").clicked()
                    {
                        go = Some(home);
                    }
                    let field = ui.add(
                        egui::TextEdit::singleline(&mut self.typed)
                            .desired_width(f32::INFINITY)
                            .hint_text("folder"),
                    );
                    field.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Folder")
                    });
                    if field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                        go = Some(PathBuf::from(self.typed.trim()));
                    }
                });
                ui.add_space(4.0);

                egui::Frame::group(ui.style()).show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .min_scrolled_height(300.0)
                        .auto_shrink(false)
                        .show(ui, |ui| {
                            if self.entries.is_empty() {
                                ui.weak("Nothing here that could be chosen.");
                            }
                            for entry in &self.entries {
                                let label = match entry.folder {
                                    true => format!("🗀  {}", entry.name),
                                    false => format!("     {}", entry.name),
                                };
                                let chosen = !entry.folder && self.name == entry.name;
                                let row = ui.selectable_label(chosen, label);
                                row.widget_info(|| {
                                    let kind = egui::WidgetType::SelectableLabel;
                                    egui::WidgetInfo::selected(kind, true, chosen, &entry.name)
                                });
                                match (entry.folder, row.clicked(), row.double_clicked()) {
                                    (true, true, _) => go = Some(self.folder.join(&entry.name)),
                                    (false, _, true) => {
                                        self.name.clone_from(&entry.name);
                                        confirmed = true;
                                    }
                                    (false, true, _) => {
                                        self.name.clone_from(&entry.name);
                                        self.problem = None;
                                    }
                                    _ => {}
                                }
                            }
                        });
                });

                ui.add_space(4.0);
                let mut entered = false;
                ui.horizontal(|ui| {
                    ui.label("Name");
                    let field = ui.add(
                        egui::TextEdit::singleline(&mut self.name).desired_width(f32::INFINITY),
                    );
                    field.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "File name")
                    });
                    entered =
                        field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
                });
                if let Some(problem) = &self.problem {
                    ui.colored_label(style::ERROR, problem);
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let confirm = ui.button(RichText::new(self.purpose.confirm()).strong());
                    confirmed |= confirm.clicked() || entered;
                    if ui.button("Cancel").clicked() {
                        answer = Answer::Cancelled;
                    }
                });
            });
        if let Some(folder) = go {
            self.enter(folder);
        }
        if confirmed {
            answer = self.confirm();
        }
        answer
    }
}

/// Where a person's own files are. Windows names it differently.
fn home() -> Option<PathBuf> {
    ["HOME", "USERPROFILE"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .find(|home| home.is_dir())
}
