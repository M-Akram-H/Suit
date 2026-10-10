//! Editors for the arguments of constructors, methods and animations: one widget per type
//! of the catalogue (spec §5.1). Each returns whether it changed what it was given.

use eframe::egui::{self, Color32, RichText};
use suit_check::{ObjectState, State};
use suit_codegen::pretty::Pretty;
use suit_core::{Arg, Args, Deck, Expr, Place, Presence, Rgba, Sym};
use suit_sig::{AnimParam, AnimParamType, Base, Constant, Param, Pass, Signature, Type};

use crate::style;

/// What an editor may refer to: the catalogue, and the objects that exist at this point.
pub struct Scope<'a> {
    pub sig: &'a Signature,
    pub deck: &'a Deck,
    pub objects: &'a [ObjectState],
}

impl Scope<'_> {
    fn pretty(&self) -> Pretty<'_> {
        Pretty {
            deck: self.deck,
            sig: self.sig,
        }
    }

    /// The kind of the object a place names, as far as it is known here.
    pub fn kind_of(&self, place: &Place) -> Option<&Sym> {
        let object = self
            .objects
            .iter()
            .find(|object| object.var == place.root)?;
        place.path.is_empty().then_some(&object.kind)
    }
}

/// One parameter, whichever of the catalogue's tables it came from.
pub struct Slot {
    pub name: Sym,
    pub ty: Type,
    pub required: bool,
    pub variadic: bool,
}

impl From<&Param> for Slot {
    fn from(param: &Param) -> Slot {
        Slot {
            name: param.name.clone(),
            ty: param.ty.clone(),
            required: param.required,
            variadic: param.pass == Pass::Variadic,
        }
    }
}

impl From<&AnimParam> for Slot {
    fn from(param: &AnimParam) -> Slot {
        let ty = match &param.ty {
            AnimParamType::Object { kind, .. } => Type::Mob(kind.clone()),
            AnimParamType::Value(ty) => ty.clone(),
        };
        Slot {
            name: param.name.clone(),
            ty,
            required: param.required,
            variadic: param.pass == Pass::Variadic,
        }
    }
}

/// A parameter name as a person would say it.
pub fn label(name: &str) -> String {
    let known = match name {
        "mobject" | "vmobject" => "object",
        "target_mobject" => "becomes",
        "tex_strings" => "formula",
        "point_or_mobject" | "mobject_or_point" => "where",
        "vectors" => "by",
        "scale_factor" => "factor",
        "filename_or_array" => "file",
        "mobjects" => "objects",
        "num_decimal_places" => "decimals",
        _ => "",
    };
    match known {
        "" => name.replace('_', " "),
        known => known.to_owned(),
    }
}

/// Edits `args` against the parameters `slots`: a row per argument, and a menu of the
/// settings that could still be added.
pub fn args(ui: &mut egui::Ui, scope: &Scope, slots: &[Slot], args: &mut Args) -> bool {
    let mut changed = false;
    let mut removed = None;
    egui::Grid::new("arguments")
        .num_columns(3)
        .spacing([8.0, 5.0])
        .show(ui, |ui| {
            for slot in slots {
                let given: Vec<usize> = (0..args.len())
                    .filter(|&index| args[index].0 == slot.name)
                    .collect();
                for (nth, &index) in given.iter().enumerate() {
                    ui.label(label(&slot.name));
                    ui.push_id((slot.name.as_str(), nth), |ui| {
                        changed |= value(ui, scope, &slot.ty, &mut args[index].1);
                    });
                    // What a constructor cannot do without stays, unless it is one of several.
                    if !slot.required || given.len() > 1 {
                        if ui
                            .small_button("×")
                            .on_hover_text("Remove this setting")
                            .clicked()
                        {
                            removed = Some(index);
                        }
                    } else {
                        ui.label("");
                    }
                    ui.end_row();
                }
                if slot.required && given.is_empty() {
                    ui.label(label(&slot.name));
                    ui.colored_label(style::ERROR, "needed");
                    ui.label("");
                    ui.end_row();
                }
            }
        });
    if let Some(index) = removed {
        args.remove(index);
        changed = true;
    }

    let missing = |slot: &&Slot| slot.variadic || !args.iter().any(|(name, _)| *name == slot.name);
    let addable: Vec<(&Slot, Arg)> = slots
        .iter()
        .filter(missing)
        .filter_map(|slot| Some((slot, starting_value(scope, slot)?)))
        .collect();
    if !addable.is_empty() {
        ui.menu_button("Add setting", |ui| {
            for (slot, start) in addable {
                if ui.button(label(&slot.name)).clicked() {
                    args.push((slot.name.clone(), start));
                    changed = true;
                    ui.close();
                }
            }
        });
    }
    changed
}

/// What a setting starts as when it is added. `None` for what cannot be edited here yet.
pub(crate) fn starting_value(scope: &Scope, slot: &Slot) -> Option<Arg> {
    let directional = ["direction", "edge", "corner", "axis", "shift", "vectors"];
    match &slot.ty {
        Type::Base(Base::Real) => Some(1.0_f64.into()),
        Type::Base(Base::Int) => Some(1_i64.into()),
        Type::Base(Base::Bool) => Some(true.into()),
        Type::Base(Base::Str) => Some("".into()),
        Type::Base(Base::Color) => Some(Expr::konst("WHITE").into()),
        Type::Base(Base::Vec) if directional.contains(&slot.name.as_str()) => {
            Some(Expr::konst("UP").into())
        }
        Type::Base(Base::Vec) => Some(Expr::vec3(0.0, 0.0, 0.0).into()),
        Type::Mob(kind) => {
            let fits = |object: &&ObjectState| scope.sig.is_subkind(&object.kind, kind);
            let object = scope.objects.iter().rev().find(fits)?;
            Some((&Place::var(object.var)).into())
        }
        Type::Union(options) if options.contains(&Type::Base(Base::Vec)) => {
            Some(Expr::vec3(0.0, 0.0, 0.0).into())
        }
        Type::Rate => Some(Arg::rate("smooth")),
        Type::Union(_) | Type::Fun(..) => None,
    }
}

/// Edits one argument according to the type its parameter has.
pub fn value(ui: &mut egui::Ui, scope: &Scope, ty: &Type, arg: &mut Arg) -> bool {
    match (ty, arg) {
        (Type::Rate, Arg::Rate(rate)) => rate_choice(ui, scope, rate),
        (Type::Base(base), Arg::Expr(expr)) => match base {
            Base::Real => real(ui, scope, expr),
            Base::Int => int(ui, scope, expr),
            Base::Bool => boolean(ui, scope, expr),
            Base::Str => text(ui, scope, expr),
            Base::Color => color(ui, scope, expr),
            Base::Vec => vector(ui, scope, expr),
        },
        (Type::Mob(kind), Arg::Expr(expr)) => object(ui, scope, kind, expr),
        (Type::Union(options), Arg::Expr(expr)) => {
            let kind = options.iter().find_map(|option| match option {
                Type::Mob(kind) => Some(kind),
                _ => None,
            });
            match kind {
                Some(kind) if options.contains(&Type::Base(Base::Vec)) => {
                    point_or_object(ui, scope, kind, expr)
                }
                _ => formula(ui, &scope.pretty().expr(expr)),
            }
        }
        (_, Arg::Object(inline)) => formula(ui, &scope.pretty().object(&inline.item)),
        (_, Arg::Fun { param, body }) => {
            let pretty = scope.pretty();
            formula(
                ui,
                &format!("{} ↦ {}", pretty.var(*param), pretty.expr(body)),
            )
        }
        (_, Arg::Expr(expr)) => formula(ui, &scope.pretty().expr(expr)),
        (_, Arg::Rate(rate)) => formula(ui, rate),
    }
}

/// Something written as a formula, which this version shows but cannot edit.
fn formula(ui: &mut egui::Ui, text: &str) -> bool {
    ui.label(RichText::new(text).monospace())
        .on_hover_text("This is a formula. It can be read here, but not edited yet.");
    false
}

fn real(ui: &mut egui::Ui, scope: &Scope, expr: &mut Expr) -> bool {
    let mut number = match expr {
        Expr::Real(number) => *number,
        Expr::Int(number) => *number as f64,
        other => return formula(ui, &scope.pretty().expr(other)),
    };
    let changed = ui
        .add(
            egui::DragValue::new(&mut number)
                .speed(0.02)
                .max_decimals(3),
        )
        .changed();
    if changed {
        *expr = Expr::Real(number);
    }
    changed
}

fn int(ui: &mut egui::Ui, scope: &Scope, expr: &mut Expr) -> bool {
    match expr {
        Expr::Int(number) => ui.add(egui::DragValue::new(number).speed(0.1)).changed(),
        other => formula(ui, &scope.pretty().expr(other)),
    }
}

fn boolean(ui: &mut egui::Ui, scope: &Scope, expr: &mut Expr) -> bool {
    match expr {
        Expr::Bool(flag) => ui.checkbox(flag, "").changed(),
        other => formula(ui, &scope.pretty().expr(other)),
    }
}

fn text(ui: &mut egui::Ui, scope: &Scope, expr: &mut Expr) -> bool {
    match expr {
        Expr::Str(text) => ui
            .add(egui::TextEdit::singleline(text).desired_width(190.0))
            .changed(),
        other => formula(ui, &scope.pretty().expr(other)),
    }
}

/// The named colours people reach for first; the rest follow in alphabetical order.
const FIRST_COLOURS: [&str; 13] = [
    "WHITE", "GRAY", "BLACK", "RED", "ORANGE", "YELLOW", "GREEN", "TEAL", "BLUE", "PURPLE", "PINK",
    "GOLD", "MAROON",
];

fn color(ui: &mut egui::Ui, scope: &Scope, expr: &mut Expr) -> bool {
    let named = |name: &str| match scope.sig.consts.get(name) {
        Some(Constant::Color(rgba)) => Some(*rgba),
        _ => None,
    };
    let (current, shown) = match expr {
        Expr::Const(name) => match named(name) {
            Some(rgba) => (rgba, name.to_string()),
            None => return formula(ui, name),
        },
        Expr::Color(rgba) => (*rgba, rgba.hex()),
        other => return formula(ui, &scope.pretty().expr(other)),
    };
    let mut changed = false;
    ui.horizontal(|ui| {
        // The swatch is also a picker, for colours that have no name.
        let [r, g, b, a] = current.0;
        let mut picked = Color32::from_rgba_unmultiplied(r, g, b, a);
        if ui.color_edit_button_srgba(&mut picked).changed() {
            *expr = Expr::Color(Rgba(picked.to_srgba_unmultiplied()));
            changed = true;
        }
        egui::ComboBox::from_id_salt("colour")
            .selected_text(shown.clone())
            .show_ui(ui, |ui| {
                let rest = scope
                    .sig
                    .consts
                    .keys()
                    .map(Sym::as_str)
                    .filter(|name| !FIRST_COLOURS.contains(name));
                for name in FIRST_COLOURS.into_iter().chain(rest) {
                    if named(name).is_some() && ui.selectable_label(shown == name, name).clicked() {
                        *expr = Expr::konst(name);
                        changed = true;
                    }
                }
            });
    });
    changed
}

const DIRECTIONS: [&str; 9] = [
    "UP", "DOWN", "LEFT", "RIGHT", "UL", "UR", "DL", "DR", "ORIGIN",
];

/// The three ways a point or displacement is usually written.
#[derive(Clone, Copy, PartialEq)]
enum VectorForm {
    Direction,
    Scaled,
    Point,
}

fn vector(ui: &mut egui::Ui, scope: &Scope, expr: &mut Expr) -> bool {
    let direction_of = |expr: &Expr| match expr {
        Expr::Const(name) if DIRECTIONS.contains(&name.as_str()) => Some(name.clone()),
        _ => None,
    };
    let form = match &*expr {
        Expr::Vec3(parts)
            if parts
                .iter()
                .all(|part| matches!(part, Expr::Real(_) | Expr::Int(_))) =>
        {
            VectorForm::Point
        }
        Expr::Call { fun, args } if &**fun == "*" => match args.as_slice() {
            [Expr::Real(_), direction] if direction_of(direction).is_some() => VectorForm::Scaled,
            _ => return formula(ui, &scope.pretty().expr(expr)),
        },
        other if direction_of(other).is_some() => VectorForm::Direction,
        other => return formula(ui, &scope.pretty().expr(other)),
    };

    let mut changed = false;
    ui.horizontal(|ui| {
        let mut wanted = form;
        let names = [
            (VectorForm::Direction, "direction"),
            (VectorForm::Scaled, "distance"),
            (VectorForm::Point, "point"),
        ];
        let current = names
            .iter()
            .find(|(candidate, _)| *candidate == form)
            .map_or("", |(_, name)| name);
        egui::ComboBox::from_id_salt("form")
            .selected_text(current)
            .width(78.0)
            .show_ui(ui, |ui| {
                for (candidate, name) in names {
                    ui.selectable_value(&mut wanted, candidate, name);
                }
            });
        if wanted != form {
            *expr = convert(scope, expr, wanted);
            changed = true;
            return;
        }
        match expr {
            Expr::Vec3(parts) => {
                for (part, axis) in parts.iter_mut().zip(["x", "y"]) {
                    ui.weak(axis);
                    changed |= real(ui, scope, part);
                }
            }
            Expr::Call { args, .. } => {
                if let [amount, direction] = args.as_mut_slice() {
                    changed |= real(ui, scope, amount);
                    ui.weak("×");
                    changed |= direction_choice(ui, direction);
                }
            }
            direction => changed |= direction_choice(ui, direction),
        }
    });
    changed
}

fn direction_choice(ui: &mut egui::Ui, expr: &mut Expr) -> bool {
    let Expr::Const(current) = expr else {
        return false;
    };
    let mut changed = false;
    egui::ComboBox::from_id_salt("direction")
        .selected_text(current.as_str())
        .width(72.0)
        .show_ui(ui, |ui| {
            for name in DIRECTIONS {
                if ui.selectable_label(&**current == name, name).clicked() {
                    *current = name.into();
                    changed = true;
                }
            }
        });
    changed
}

/// The same vector written in another form, as far as the forms allow.
fn convert(scope: &Scope, expr: &Expr, to: VectorForm) -> Expr {
    let constant = |name: &str| match scope.sig.consts.get(name) {
        Some(Constant::Vec(vector)) => *vector,
        _ => [0.0; 3],
    };
    let (direction, amount, point) = match expr {
        Expr::Const(name) => (name.to_string(), 1.0, constant(name)),
        Expr::Call { args, .. } => match args.as_slice() {
            [Expr::Real(amount), Expr::Const(name)] => (
                name.to_string(),
                *amount,
                constant(name).map(|c| c * amount),
            ),
            _ => ("UP".to_owned(), 1.0, [0.0; 3]),
        },
        Expr::Vec3(parts) => {
            let number = |part: &Expr| match part {
                Expr::Real(number) => *number,
                Expr::Int(number) => *number as f64,
                _ => 0.0,
            };
            (
                "UP".to_owned(),
                1.0,
                [number(&parts[0]), number(&parts[1]), number(&parts[2])],
            )
        }
        _ => ("UP".to_owned(), 1.0, [0.0; 3]),
    };
    match to {
        VectorForm::Direction => Expr::konst(&direction),
        VectorForm::Scaled => Expr::call("*", vec![Expr::Real(amount), Expr::konst(&direction)]),
        VectorForm::Point => Expr::vec3(point[0], point[1], point[2]),
    }
}

/// Chooses one of the objects that exist here and are of `kind`.
pub fn object(ui: &mut egui::Ui, scope: &Scope, kind: &Sym, expr: &mut Expr) -> bool {
    let current = match expr {
        Expr::Place(place) => Some(place.clone()),
        Expr::Var(var) => Some(Place::var(*var)),
        _ => None,
    };
    let shown = current
        .as_ref()
        .map_or("choose…".to_owned(), |place| scope.pretty().place(place));
    let mut changed = false;
    egui::ComboBox::from_id_salt("object")
        .selected_text(shown)
        .show_ui(ui, |ui| {
            let fitting = scope
                .objects
                .iter()
                .filter(|object| scope.sig.is_subkind(&object.kind, kind));
            let mut any = false;
            for candidate in fitting {
                any = true;
                let place = Place::var(candidate.var);
                let mut name = scope.pretty().place(&place);
                match candidate.state {
                    State::Is(Presence::On) => {}
                    State::Is(Presence::Off) => name.push_str("  (not on the slide)"),
                    State::Is(Presence::Spent) => name.push_str("  (used up)"),
                    State::Mixed => name.push_str("  (partly on the slide)"),
                }
                if ui
                    .selectable_label(current.as_ref() == Some(&place), name)
                    .clicked()
                {
                    *expr = Expr::Place(place);
                    changed = true;
                }
            }
            if !any {
                ui.weak(format!("There is no {kind} here yet."));
            }
        });
    changed
}

/// A position given either as a point or as "where that object is".
fn point_or_object(ui: &mut egui::Ui, scope: &Scope, kind: &Sym, expr: &mut Expr) -> bool {
    let is_object = matches!(expr, Expr::Place(_) | Expr::Var(_));
    let mut wants_object = is_object;
    let mut changed = false;
    ui.horizontal(|ui| {
        let shown = if is_object { "object" } else { "position" };
        egui::ComboBox::from_id_salt("anchor")
            .selected_text(shown)
            .width(74.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut wants_object, true, "object");
                ui.selectable_value(&mut wants_object, false, "position");
            });
        if wants_object != is_object {
            let fits = |object: &&ObjectState| scope.sig.is_subkind(&object.kind, kind);
            *expr = match scope.objects.iter().rev().find(fits) {
                Some(object) if wants_object => Expr::Place(Place::var(object.var)),
                _ => Expr::vec3(0.0, 0.0, 0.0),
            };
            changed = true;
        } else if is_object {
            changed |= object(ui, scope, kind, expr);
        } else {
            changed |= vector(ui, scope, expr);
        }
    });
    changed
}

fn rate_choice(ui: &mut egui::Ui, scope: &Scope, rate: &mut Sym) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt("rate")
        .selected_text(rate.as_str())
        .show_ui(ui, |ui| {
            for name in scope.sig.rates.keys() {
                if ui.selectable_label(*rate == *name, name.as_str()).clicked() {
                    *rate = name.clone();
                    changed = true;
                }
            }
        });
    changed
}
