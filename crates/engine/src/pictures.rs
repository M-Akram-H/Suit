//! Where a deck's pictures are. A deck names a picture relative to its own folder when the
//! picture lies in it, so that the folder can be moved or sent as a whole; Manim, which runs
//! somewhere else, is given the same deck with every name made whole.

use std::{
    fs,
    path::{Path, PathBuf},
};

use suit_core::{Arg, Args, Command, Deck, Expr, Node, Object, SlideContent};

use crate::Session;

/// Calls `visit` on the file name of every picture in the deck.
fn for_each_picture(deck: &mut Deck, visit: &mut impl FnMut(&mut String)) {
    fn in_args(args: &mut Args, visit: &mut impl FnMut(&mut String)) {
        for (_, arg) in args {
            if let Arg::Object(object) = arg {
                in_object(object, visit);
            }
        }
    }
    fn in_object(node: &mut Node<Object>, visit: &mut impl FnMut(&mut String)) {
        match &mut node.item {
            Object::New { ctor, args } => {
                if &**ctor == "ImageMobject" {
                    for (name, arg) in args.iter_mut() {
                        if let (true, Arg::Expr(Expr::Str(file))) = (&**name == FILE, arg) {
                            visit(file);
                        }
                    }
                }
                in_args(args, visit);
            }
            Object::Chain { base, call } => {
                in_object(base, visit);
                in_args(&mut call.args, visit);
            }
            Object::Group { .. } | Object::Copy(_) => {}
        }
    }
    for slide in &mut deck.slides {
        let SlideContent::Blueprint(blueprint) = &mut slide.content else {
            continue;
        };
        for node in &mut blueprint.body {
            if let Command::LetObject { value, .. } = &mut node.item {
                in_object(value, visit);
            }
        }
    }
}

/// The parameter of a picture that says which file it is.
const FILE: &str = "filename_or_array";

/// `file` as a deck in `folder` names it.
fn named(folder: Option<&Path>, file: &Path) -> String {
    let inside = folder.and_then(|folder| file.strip_prefix(folder).ok());
    inside.unwrap_or(file).display().to_string()
}

/// The file a deck in `folder` means by `name`.
fn meant(folder: Option<&Path>, name: &str) -> PathBuf {
    match folder {
        Some(folder) if Path::new(name).is_relative() => folder.join(name),
        _ => PathBuf::from(name),
    }
}

/// Names every picture of `deck` as a deck in `to` would, given that it is in `from` now.
pub(crate) fn rebase(deck: &mut Deck, from: Option<&Path>, to: Option<&Path>) {
    for_each_picture(deck, &mut |file| *file = named(to, &meant(from, file)));
}

impl Session {
    /// The folder the deck is in, if it has been saved.
    fn folder(&self) -> Option<PathBuf> {
        Some(self.path.as_ref()?.parent()?.to_path_buf())
    }

    /// `file` as the deck names it: relative to its folder if it lies in there.
    pub(crate) fn name_of_picture(&self, file: &Path) -> String {
        named(self.folder().as_deref(), file)
    }

    /// The deck as Manim is to see it: with every picture named by its whole path.
    pub(crate) fn deck_for_manim(&self) -> Deck {
        let mut deck = self.deck.clone();
        rebase(&mut deck, self.folder().as_deref(), None);
        deck
    }

    /// The pictures that are not in the deck's folder, which a copy of that folder would
    /// be without.
    pub fn pictures_elsewhere(&self) -> Vec<PathBuf> {
        let folder = self.folder();
        let mut elsewhere = Vec::new();
        for_each_picture(&mut self.deck.clone(), &mut |file| {
            let path = meant(folder.as_deref(), file);
            let inside = folder
                .as_ref()
                .is_some_and(|folder| path.starts_with(folder));
            if !inside && !elsewhere.contains(&path) {
                elsewhere.push(path);
            }
        });
        elsewhere
    }

    /// Copies every picture that is not in the deck's folder into a `pictures` folder
    /// there, and has the deck use the copies. Returns how many were copied.
    pub fn gather_pictures(&mut self) -> Result<usize, String> {
        let folder = self
            .folder()
            .ok_or("Save the deck first: the pictures go next to its file.")?;
        let elsewhere = self.pictures_elsewhere();
        if elsewhere.is_empty() {
            return Ok(0);
        }
        let target = folder.join("pictures");
        fs::create_dir_all(&target)
            .map_err(|e| format!("cannot create {}: {e}", target.display()))?;
        let mut copies = Vec::new();
        for file in &elsewhere {
            let contents =
                fs::read(file).map_err(|e| format!("cannot read {}: {e}", file.display()))?;
            let name = file.file_name().unwrap_or_default().to_string_lossy();
            let (stem, ending) = name.rsplit_once('.').unwrap_or((&name, ""));
            // A name that is free, or that is already this very picture.
            let copy = (1..)
                .map(|n| match n {
                    1 => target.join(&*name),
                    n => target.join(format!("{stem} {n}.{ending}")),
                })
                .find(|copy| fs::read(copy).map_or(true, |there| there == contents))
                .expect("there are always more numbers");
            fs::write(&copy, contents)
                .map_err(|e| format!("cannot write {}: {e}", copy.display()))?;
            copies.push((file.clone(), copy));
        }
        let count = copies.len();
        self.edit_pictures("Gather pictures", |path| {
            let copy = copies.iter().find(|(file, _)| file == path);
            copy.map(|(_, copy)| copy.clone())
        });
        Ok(count)
    }

    /// Points pictures at other files, as one undo step: `moved` says where a picture now
    /// is, or `None` to leave it.
    fn edit_pictures(&mut self, label: &str, moved: impl Fn(&Path) -> Option<PathBuf>) {
        let folder = self.folder();
        self.change_deck(label, |deck| {
            for_each_picture(deck, &mut |file| {
                if let Some(now) = moved(&meant(folder.as_deref(), file)) {
                    *file = named(folder.as_deref(), &now);
                }
            });
        });
    }
}
