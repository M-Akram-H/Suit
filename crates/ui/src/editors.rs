//! Editors for the arguments of constructors, methods and animations: one widget per type
//! of the catalogue (spec §5.1). Each returns whether it changed what it was given.

use eframe::egui::{self, Color32, RichText};
use suit_check::{ObjectState, State};
use suit_codegen::{
    formula::{self, Names},
    pretty::Pretty,
};
use suit_core::{Arg, Args, Deck, Expr, Place, Presence, Rgba, Sym, UNBOUND, VarId};
use suit_engine::stretches;
use suit_render::Remark;
use suit_sig::{AnimParam, AnimParamType, Base, Constant, Param, Pass, Signature, Type};

use crate::style;

/// What an editor may refer to: the catalogue, and the objects and values that exist at
/// this point.
#[derive(Clone)]
pub struct Scope<'a> {
    pub sig: &'a Signature,
    pub deck: &'a Deck,
    pub objects: &'a [ObjectState],
    /// The same objects, as formulas name them.
    named: Vec<VarId>,
    /// The values a formula may read here: the named ones, then the parameters of whatever
    /// is being edited.
    values: Vec<VarId>,
    /// What Manim remarked on when it last ran the deck: what graphs found, for one.
    remarks: &'a [Remark],
}

impl<'a> Scope<'a> {
    pub fn new(
        sig: &'a Signature,
        deck: &'a Deck,
        objects: &'a [ObjectState],
        values: Vec<VarId>,
    ) -> Scope<'a> {
        Scope {
            sig,
            deck,
            objects,
            named: objects.iter().map(|object| object.var).collect(),
            values,
            remarks: &[],
        }
    }

    /// The same, knowing what Manim remarked on.
    pub fn knowing(mut self, remarks: &'a [Remark]) -> Scope<'a> {
        self.remarks = remarks;
        self
    }

    /// What the graph `var` found out about its rule, if Manim has made it.
    fn found_by(&self, var: VarId) -> Option<&'a Remark> {
        let remarks = self.remarks.iter().rev();
        remarks
            .into_iter()
            .find(|remark| remark.var == var && remark.what == "plot")
    }

    /// The same, inside something that has `parameter`: a function, or a step that is
    /// repeated with the time passed.
    pub fn inside(&self, parameter: VarId) -> Scope<'a> {
        let mut scope = self.clone();
        scope.values.push(parameter);
        scope
    }

    fn names(&self) -> Names<'_> {
        Names {
            deck: self.deck,
            sig: self.sig,
            objects: &self.named,
            values: &self.values,
        }
    }

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
    /// For a range: whether it is the range a rule is worked out over, point by point,
    /// rather than the range of an axis with its ticks.
    pub sampled: bool,
}

impl Slot {
    fn is_range(&self) -> bool {
        self.name.ends_with("_range") && self.ty == Type::Base(Base::Vec)
    }
}

impl From<&Param> for Slot {
    fn from(param: &Param) -> Slot {
        Slot {
            name: param.name.clone(),
            ty: param.ty.clone(),
            required: param.required,
            variadic: matches!(param.pass, Pass::Variadic | Pass::List),
            sampled: false,
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
            sampled: false,
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
        "at" => "at x",
        "to" => "to x",
        "dot" => "mark the point",
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
                        changed |= match &mut args[index].1 {
                            Arg::Expr(expr) if slot.is_range() => range(ui, scope, expr),
                            arg => value(ui, scope, &slot.ty, arg),
                        };
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
        // Somewhere else than where a new tangent is, so that going there is seen.
        Type::Base(Base::Real) if &*slot.name == "to" => Some(3.0_f64.into()),
        Type::Base(Base::Real) => Some(1.0_f64.into()),
        Type::Base(Base::Int) => Some(1_i64.into()),
        Type::Base(Base::Bool) => Some(true.into()),
        Type::Base(Base::Str) => Some("".into()),
        Type::Base(Base::Color) => Some(Expr::konst("WHITE").into()),
        Type::Base(Base::Vec) if directional.contains(&slot.name.as_str()) => {
            Some(Expr::konst("UP").into())
        }
        // Across the frame in fine steps for a rule; a few whole ticks for an axis.
        Type::Base(Base::Vec) if slot.is_range() => Some(match (slot.sampled, &*slot.name) {
            (true, "t_range") => Expr::vec3(0.0, 1.0, 0.01).into(),
            (true, _) => Expr::vec3(-7.0, 7.0, 0.05).into(),
            (false, "y_range") => Expr::vec3(-3.0, 3.0, 1.0).into(),
            (false, _) => Expr::vec3(-5.0, 5.0, 1.0).into(),
        }),
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
        // The identity, or a point that moves to the right; its parameter gets a variable
        // of its own when the step goes into the deck.
        Type::Fun(_, Base::Vec) => {
            let along = [Expr::Var(UNBOUND), Expr::Real(0.0), Expr::Real(0.0)];
            Some(Arg::fun(UNBOUND, Expr::Vec3(Box::new(along))))
        }
        Type::Fun(..) => Some(Arg::fun(UNBOUND, Expr::Var(UNBOUND))),
        Type::Union(_) => Some(0.0_f64.into()),
    }
}

/// Edits one argument according to the type its parameter has.
pub fn value(ui: &mut egui::Ui, scope: &Scope, ty: &Type, arg: &mut Arg) -> bool {
    match arg {
        Arg::Rate(rate) => rate_choice(ui, scope, rate),
        Arg::Object(inline) => {
            ui.label(RichText::new(scope.pretty().object(&inline.item)).monospace());
            false
        }
        Arg::Fun { param, body } => {
            let mut changed = false;
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{} ↦", scope.pretty().var(*param))).monospace())
                    .on_hover_text("A rule: what comes out for each value that goes in.");
                changed = formula(ui, &scope.inside(*param), body);
            });
            changed
        }
        // An object is chosen, never computed.
        Arg::Expr(expr) => match ty {
            Type::Mob(kind) => object(ui, scope, kind, expr),
            _ => expression(ui, scope, ty, expr),
        },
    }
}

/// A value: with the widget for its type where it is a plain one, and as a formula where
/// it is computed or where that was asked for with the `fx` button.
fn expression(ui: &mut egui::Ui, scope: &Scope, ty: &Type, expr: &mut Expr) -> bool {
    let asked = ui.id().with("as a formula");
    let mut as_formula = ui.data(|data| data.get_temp::<bool>(asked).unwrap_or(false));
    let mut changed = false;
    ui.horizontal(|ui| {
        changed = match as_formula {
            true => formula(ui, scope, expr),
            false => typed(ui, scope, ty, expr),
        };
        let shown = as_formula || !is_plain(expr);
        let hint = match shown {
            true => {
                "This is a formula: it can use numbers, names of objects and values, \
                     and what objects can be asked, like circle.center or t.value."
            }
            false => "Write this as a formula instead, to compute it from other things.",
        };
        if ui
            .selectable_label(shown, RichText::new("fx").small())
            .on_hover_text(hint)
            .clicked()
        {
            as_formula = !as_formula;
        }
    });
    ui.data_mut(|data| data.insert_temp(asked, as_formula));
    changed
}

/// Whether `expr` is written out rather than computed.
fn is_plain(expr: &Expr) -> bool {
    let numeral = |expr: &Expr| matches!(expr, Expr::Real(_) | Expr::Int(_));
    match expr {
        Expr::Vec3(parts) => parts.iter().all(numeral),
        Expr::Call { fun, args } if &**fun == "*" => {
            matches!(args.as_slice(), [amount, Expr::Const(_)] if numeral(amount))
        }
        Expr::If { .. } | Expr::Query { .. } | Expr::Call { .. } => false,
        _ => true,
    }
}

/// The widget for a value of type `ty`. Each falls back to a formula for what it cannot show.
fn typed(ui: &mut egui::Ui, scope: &Scope, ty: &Type, expr: &mut Expr) -> bool {
    match ty {
        Type::Base(Base::Real) => real(ui, scope, expr),
        Type::Base(Base::Int) => int(ui, scope, expr),
        Type::Base(Base::Bool) => boolean(ui, scope, expr),
        Type::Base(Base::Str) => text(ui, scope, expr),
        Type::Base(Base::Color) => color(ui, scope, expr),
        Type::Base(Base::Vec) => vector(ui, scope, expr),
        Type::Mob(kind) => object(ui, scope, kind, expr),
        Type::Union(options) => {
            let kind = options.iter().find_map(|option| match option {
                Type::Mob(kind) => Some(kind),
                _ => None,
            });
            match kind {
                Some(kind) if options.contains(&Type::Base(Base::Vec)) => {
                    point_or_object(ui, scope, kind, expr)
                }
                _ => formula(ui, scope, expr),
            }
        }
        Type::Fun(..) | Type::Rate => formula(ui, scope, expr),
    }
}

/// A formula, typed in. What is typed is taken as soon as it is a formula; until then the
/// field says what is wrong with it and the value stays as it was.
pub fn formula(ui: &mut egui::Ui, scope: &Scope, expr: &mut Expr) -> bool {
    let names = scope.names();
    let id = ui.id().with("formula");
    let field = id.with("field");
    // What is in the field, and what is the matter with it.
    let kept = ui.data(|data| data.get_temp::<(String, Option<String>)>(id));
    let typing = ui.memory(|memory| memory.has_focus(field));
    // While nobody is typing, the field shows the formula as it is.
    let (mut text, mut problem) = kept
        .filter(|_| typing)
        .unwrap_or_else(|| (formula::write(expr, &names), None));
    let mut changed = false;
    ui.vertical(|ui| {
        // A field takes the room it is given, and a table gives a column the room its
        // fields took: without a floor the two settle on next to nothing.
        ui.set_min_width(200.0);
        let mut output = egui::TextEdit::singleline(&mut text)
            .id(field)
            .font(egui::TextStyle::Monospace)
            .desired_width(200.0)
            .show(ui);
        take_the_keyboard(ui, &mut output, text.chars().count());
        if output.response.response.changed() {
            match formula::parse(&text, &names) {
                Ok(read) => {
                    changed = read != *expr;
                    *expr = read;
                    problem = None;
                }
                Err(mistake) => problem = Some(mistake.message),
            }
        }
        if let Some(problem) = &problem {
            ui.colored_label(style::ERROR, problem);
        }
    });
    ui.data_mut(|data| data.insert_temp(id, (text, problem)));
    changed
}

fn real(ui: &mut egui::Ui, scope: &Scope, expr: &mut Expr) -> bool {
    let mut number = match expr {
        Expr::Real(number) => *number,
        Expr::Int(number) => *number as f64,
        other => return formula(ui, scope, other),
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
        other => formula(ui, scope, other),
    }
}

fn boolean(ui: &mut egui::Ui, scope: &Scope, expr: &mut Expr) -> bool {
    match expr {
        Expr::Bool(flag) => ui.checkbox(flag, "").changed(),
        other => formula(ui, scope, other),
    }
}

/// Under this name in the window's memory: when a text setting was asked to take the
/// keyboard.
const TYPE_HERE: &str = "type here";

/// Has the first text or formula drawn next take the keyboard with all of it selected, so
/// that what is typed replaces it: for a text or a graph that has just been put on the
/// slide, or that was double-clicked.
pub fn type_into_the_text(ctx: &egui::Context) {
    let now = ctx.input(|input| input.time);
    ctx.data_mut(|data| data.insert_temp(egui::Id::new(TYPE_HERE), now));
}

/// Gives the field just drawn the keyboard, with its `length` characters selected, if
/// that was asked for a moment ago and no field before it has taken it.
fn take_the_keyboard(ui: &egui::Ui, output: &mut egui::text_edit::TextEditOutput, length: usize) {
    let now = ui.input(|input| input.time);
    let asked = ui.data_mut(|data| data.remove_temp::<f64>(egui::Id::new(TYPE_HERE)));
    // A wish that found no field at the time has lapsed.
    if asked.is_some_and(|at| now - at < 1.0) {
        let response = &output.response.response;
        response.request_focus();
        let all = egui::text::CCursorRange::two(
            egui::text::CCursor::new(0),
            egui::text::CCursor::new(length),
        );
        output.state.cursor.set_char_range(Some(all));
        output.state.clone().store(ui.ctx(), response.id);
    }
}

fn text(ui: &mut egui::Ui, scope: &Scope, expr: &mut Expr) -> bool {
    match expr {
        Expr::Str(text) => {
            let mut output = egui::TextEdit::singleline(text)
                .desired_width(190.0)
                .show(ui);
            take_the_keyboard(ui, &mut output, text.chars().count());
            output.response.response.changed()
        }
        other => formula(ui, scope, other),
    }
}

/// The settings of a graph that are its own: where it is drawn, and its asymptotes. The
/// rule and the rest are settings like any other. `var` is the graph, for saying what
/// Manim found when it made it.
pub fn graph(ui: &mut egui::Ui, scope: &Scope, args: &mut Args, var: Option<VarId>) -> bool {
    let names = scope.names();
    let found = var.and_then(|var| scope.found_by(var));
    /// Whether an argument is the one called `name`.
    fn is(name: &str) -> impl Fn(&(Sym, Arg)) -> bool + '_ {
        move |(given, _)| &**given == name
    }
    let mut changed = false;

    // Where: stretches written by hand, or nothing for wherever the rule has a value.
    let given: Vec<(Expr, Expr)> = args
        .iter()
        .filter(|arg| is("on")(arg))
        .filter_map(|(_, arg)| match arg {
            Arg::Expr(Expr::Vec3(parts)) => Some((parts[0].clone(), parts[1].clone())),
            _ => None,
        })
        .collect();
    let id = ui.id().with("drawn on");
    let field = id.with("field");
    let kept = ui.data(|data| data.get_temp::<(String, Option<String>)>(id));
    let typing = ui.memory(|memory| memory.has_focus(field));
    let (mut text, mut problem) = kept
        .filter(|_| typing)
        .unwrap_or_else(|| (formula::write_stretches(&given, &names), None));
    ui.horizontal(|ui| {
        ui.label("drawn on");
        let edit = egui::TextEdit::singleline(&mut text)
            .id(field)
            .font(egui::TextStyle::Monospace)
            .hint_text("wherever it has a value")
            .desired_width(200.0);
        let response = ui.add(edit).on_hover_text(
            "Leave this empty and the graph is drawn wherever its rule has a value. Or say \
             where yourself: [0.1, 7], or several stretches, [-1, -0.1] ∪ [0.1, 1].",
        );
        if response.changed() {
            match formula::parse_stretches(&text, &names) {
                Ok(read) => {
                    changed |= read != given;
                    args.retain(|arg| !is("on")(arg));
                    let stretch =
                        |(from, to)| Arg::Expr(Expr::Vec3(Box::new([from, to, Expr::Real(0.0)])));
                    args.extend(read.into_iter().map(|ends| ("on".into(), stretch(ends))));
                    problem = None;
                }
                Err(mistake) => problem = Some(mistake.message),
            }
        }
    });
    if let Some(problem) = &problem {
        ui.colored_label(style::ERROR, problem);
    }
    ui.data_mut(|data| data.insert_temp(id, (text, problem)));
    if let Some(found) = found {
        if found.pieces.is_empty() {
            let nowhere = "The rule has no value anywhere here, so nothing is drawn.";
            ui.colored_label(style::WARNING, nowhere);
        } else if found.missing || found.pieces != found.asked {
            ui.weak(format!(
                "Drawn on {}: where the rule has a value near the picture.",
                stretches(&found.pieces)
            ));
        }
    }

    // Asymptotes: found whether shown or not, so that the box can say what it would show.
    let mut shown = args
        .iter()
        .any(|(name, arg)| &**name == "asymptotes" && *arg == Arg::Expr(Expr::Bool(true)));
    ui.horizontal(|ui| {
        if ui.checkbox(&mut shown, "Show asymptotes").changed() {
            args.retain(|arg| !is("asymptotes")(arg));
            if shown {
                args.push(("asymptotes".into(), true.into()));
            }
            changed = true;
        }
        if let Some(found) = found {
            let number = |value: &f64| format!("{}", (value * 1000.0).round() / 1000.0 + 0.0);
            let lines = found.vertical.iter().map(|x| format!("x = {}", number(x)));
            let levels = found
                .horizontal
                .iter()
                .map(|y| format!("y = {}", number(y)));
            let all: Vec<String> = lines.chain(levels).collect();
            ui.weak(match all.is_empty() {
                true => "none found".to_owned(),
                false => format!("found: {}", all.join(", ")),
            });
        }
    });
    changed
}

/// A range: where it starts, where it ends, and how far apart the points in it are.
fn range(ui: &mut egui::Ui, scope: &Scope, expr: &mut Expr) -> bool {
    let plain = |part: &Expr| matches!(part, Expr::Real(_) | Expr::Int(_));
    let Expr::Vec3(parts) = expr else {
        return formula(ui, scope, expr);
    };
    if !parts.iter().all(plain) {
        return formula(ui, scope, expr);
    }
    let mut changed = false;
    ui.horizontal(|ui| {
        for (part, what) in parts.iter_mut().zip(["from", "to", "in steps of"]) {
            ui.weak(what);
            ui.push_id(what, |ui| changed |= real(ui, scope, part));
        }
    });
    changed
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
    let (current, shown) = match &*expr {
        Expr::Const(name) => match named(name) {
            Some(rgba) => (rgba, name.to_string()),
            None => return formula(ui, scope, expr),
        },
        Expr::Color(rgba) => (*rgba, rgba.hex()),
        _ => return formula(ui, scope, expr),
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
            _ => return formula(ui, scope, expr),
        },
        other if direction_of(other).is_some() => VectorForm::Direction,
        _ => return formula(ui, scope, expr),
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
