//! The starting catalogue (spec Appendix A), written against manim 0.22.0.
//!
//! Parameter names are Manim's own; `tools/check_catalog.py` verifies every entry against
//! the installed library, so a typo here fails a test instead of a render.

use std::f64::consts::{PI, TAU};

use suit_core::Rgba;

use crate::{
    AnimParam, AnimParamType, Animation, Base, Constant, Ctor, Function, GROUP_KIND, Kind, Method,
    Param, Pass, PyForm, Query, ROOT_KIND, Role, Signature, Type, VGROUP_KIND, colors::COLORS,
};

const REAL: Type = Type::Base(Base::Real);
const INT: Type = Type::Base(Base::Int);
const BOOL: Type = Type::Base(Base::Bool);
const STR: Type = Type::Base(Base::Str);
const VEC: Type = Type::Base(Base::Vec);
const COLOR: Type = Type::Base(Base::Color);

const VMOBJECT: &str = "VMobject";

fn mob(kind: &str) -> Type {
    Type::Mob(kind.into())
}

/// A point, or an object standing for its centre.
fn anchor() -> Type {
    Type::Union(vec![VEC, mob(ROOT_KIND)])
}

fn param(name: &str, ty: Type, required: bool, pass: Pass) -> Param {
    Param {
        name: name.into(),
        ty,
        required,
        pass,
    }
}

/// Optional, passed by keyword.
fn kw(name: &str, ty: Type) -> Param {
    param(name, ty, false, Pass::Keyword)
}

/// Optional, positional.
fn pos(name: &str, ty: Type) -> Param {
    param(name, ty, false, Pass::Positional)
}

/// Required, positional.
fn req(name: &str, ty: Type) -> Param {
    param(name, ty, true, Pass::Positional)
}

/// Zero or more, positional.
/// Several values of this type, passed together by name.
fn list(name: &str, ty: Type) -> Param {
    param(name, ty, false, Pass::List)
}

fn rest(name: &str, ty: Type) -> Param {
    param(name, ty, false, Pass::Variadic)
}

/// A required object parameter of an animation.
fn obj(name: &str, role: Role, kind: &str) -> AnimParam {
    AnimParam {
        name: name.into(),
        ty: AnimParamType::Object {
            role,
            kind: kind.into(),
        },
        required: true,
        pass: Pass::Positional,
    }
}

impl From<Param> for AnimParam {
    fn from(p: Param) -> AnimParam {
        AnimParam {
            name: p.name,
            ty: AnimParamType::Value(p.ty),
            required: p.required,
            pass: p.pass,
        }
    }
}

struct Builder {
    sig: Signature,
}

impl Builder {
    fn kind(&mut self, name: &str, parent: Option<&str>, elem: Option<&str>, params: Vec<Param>) {
        let kind = Kind {
            parent: parent.map(Into::into),
            elem: elem.map(Into::into),
            params,
        };
        self.sig.kinds.insert(name.into(), kind);
    }

    /// A kind together with its constructor.
    fn ctor(&mut self, name: &str, parent: &str, params: Vec<Param>) {
        self.kind(name, Some(parent), None, Vec::new());
        self.sig.ctors.insert(
            name.into(),
            Ctor {
                kind: name.into(),
                params,
            },
        );
    }

    fn method(&mut self, name: &str, receiver: &str, params: Vec<Param>) {
        let method = Method {
            receiver: receiver.into(),
            params,
            reshapes: false,
        };
        self.sig
            .methods
            .entry(name.into())
            .or_default()
            .push(method);
    }

    fn query(&mut self, name: &str, receiver: &str, params: Vec<Param>, result: Base) {
        let query = Query {
            receiver: receiver.into(),
            params,
            result,
            property: false,
        };
        self.sig.queries.entry(name.into()).or_default().push(query);
    }

    fn anim(&mut self, name: &str, objects: Vec<AnimParam>, values: Vec<Param>) -> &mut Animation {
        let params = objects
            .into_iter()
            .chain(values.into_iter().map(Into::into))
            .collect();
        let anim = Animation {
            params,
            run_time: 1.0,
            rate: "smooth".into(),
            lag: 0.0,
            weakens: false,
        };
        self.sig.anims.entry(name.into()).or_insert(anim)
    }

    fn function(&mut self, name: &str, params: &[Base], result: Base, py: PyForm) {
        let function = Function {
            params: params.to_vec(),
            result,
            py,
        };
        self.sig.fns.entry(name.into()).or_default().push(function);
    }
}

impl Signature {
    pub fn builtin() -> Signature {
        let mut b = Builder {
            sig: Signature::default(),
        };
        kinds_and_ctors(&mut b);
        methods(&mut b);
        queries(&mut b);
        animations(&mut b);
        rates(&mut b);
        functions(&mut b);
        constants(&mut b);
        b.sig
    }
}

fn kinds_and_ctors(b: &mut Builder) {
    b.kind(ROOT_KIND, None, Some(ROOT_KIND), vec![kw("color", COLOR)]);
    b.kind(
        VMOBJECT,
        Some(ROOT_KIND),
        Some(VMOBJECT),
        vec![
            kw("fill_color", COLOR),
            kw("fill_opacity", REAL),
            kw("stroke_color", COLOR),
            kw("stroke_opacity", REAL),
            kw("stroke_width", REAL),
        ],
    );
    b.kind(GROUP_KIND, Some(ROOT_KIND), None, vec![]);
    b.kind(VGROUP_KIND, Some(VMOBJECT), None, vec![]);
    b.kind("Polygram", Some(VMOBJECT), None, vec![]);

    // Shapes
    b.ctor(
        "Arc",
        VMOBJECT,
        vec![
            kw("radius", REAL),
            kw("start_angle", REAL),
            kw("angle", REAL),
            kw("arc_center", VEC),
        ],
    );
    b.ctor("Circle", "Arc", vec![kw("radius", REAL)]);
    b.ctor("Dot", "Circle", vec![pos("point", VEC), kw("radius", REAL)]);
    b.ctor(
        "Ellipse",
        "Circle",
        vec![kw("width", REAL), kw("height", REAL)],
    );
    b.ctor("Polygon", "Polygram", vec![rest("vertices", VEC)]);
    b.ctor(
        "Rectangle",
        "Polygon",
        vec![kw("width", REAL), kw("height", REAL)],
    );
    b.ctor("Square", "Rectangle", vec![kw("side_length", REAL)]);
    b.ctor(
        "RoundedRectangle",
        "Rectangle",
        vec![
            kw("corner_radius", REAL),
            kw("width", REAL),
            kw("height", REAL),
        ],
    );
    b.ctor(
        "SurroundingRectangle",
        "RoundedRectangle",
        vec![
            rest("mobjects", mob(ROOT_KIND)),
            kw("buff", REAL),
            kw("corner_radius", REAL),
        ],
    );
    b.ctor("RegularPolygon", "Polygram", vec![kw("n", INT)]);
    b.ctor("Triangle", "RegularPolygon", vec![]);
    b.ctor(
        "Star",
        "Polygon",
        vec![
            kw("n", INT),
            kw("outer_radius", REAL),
            kw("inner_radius", REAL),
        ],
    );

    // Lines
    let ends = || {
        vec![
            pos("start", anchor()),
            pos("end", anchor()),
            kw("buff", REAL),
        ]
    };
    b.ctor(
        "Line",
        VMOBJECT,
        [ends(), vec![kw("path_arc", REAL)]].concat(),
    );
    b.ctor("Arrow", "Line", ends());
    b.ctor("DoubleArrow", "Arrow", ends());
    b.ctor(
        "DashedLine",
        "Line",
        [
            ends(),
            vec![kw("dash_length", REAL), kw("dashed_ratio", REAL)],
        ]
        .concat(),
    );
    b.ctor(
        "Brace",
        VMOBJECT,
        vec![
            req("mobject", mob(ROOT_KIND)),
            kw("direction", VEC),
            kw("buff", REAL),
        ],
    );

    // Text and numbers
    b.ctor(
        "Text",
        VMOBJECT,
        vec![
            req("text", STR),
            kw("font_size", REAL),
            kw("font", STR),
            kw("weight", STR),
            kw("slant", STR),
            kw("line_spacing", REAL),
        ],
    );
    b.ctor(
        "MarkupText",
        VMOBJECT,
        vec![req("text", STR), kw("font_size", REAL), kw("font", STR)],
    );
    b.ctor(
        "MathTex",
        VMOBJECT,
        vec![rest("tex_strings", STR), kw("font_size", REAL)],
    );
    b.ctor(
        "Tex",
        "MathTex",
        vec![rest("tex_strings", STR), kw("font_size", REAL)],
    );
    b.ctor(
        "DecimalNumber",
        VMOBJECT,
        vec![
            kw("number", REAL),
            kw("num_decimal_places", INT),
            kw("include_sign", BOOL),
            kw("font_size", REAL),
        ],
    );
    b.ctor(
        "Integer",
        "DecimalNumber",
        vec![kw("number", REAL), kw("font_size", REAL)],
    );
    b.ctor("ValueTracker", ROOT_KIND, vec![kw("value", REAL)]);

    // Coordinate systems and graphs. A range is `⟨min, max, step⟩`.
    b.ctor(
        "NumberLine",
        "Line",
        vec![
            kw("x_range", VEC),
            kw("length", REAL),
            kw("include_numbers", BOOL),
            kw("include_ticks", BOOL),
            kw("include_tip", BOOL),
        ],
    );
    let plane = || {
        vec![
            kw("x_range", VEC),
            kw("y_range", VEC),
            kw("x_length", REAL),
            kw("y_length", REAL),
        ]
    };
    b.ctor(
        "Axes",
        VGROUP_KIND,
        [plane(), vec![kw("tips", BOOL)]].concat(),
    );
    b.ctor("NumberPlane", "Axes", plane());
    b.ctor(
        "ParametricFunction",
        VMOBJECT,
        vec![
            req("function", Type::Fun(Base::Real, Base::Vec)),
            kw("t_range", VEC),
        ],
    );
    b.ctor(
        "FunctionGraph",
        "ParametricFunction",
        vec![
            req("function", Type::Fun(Base::Real, Base::Real)),
            kw("x_range", VEC),
        ],
    );

    // Not Manim's: defined by the export itself (`codegen/src/plot.py`). The graph of a
    // rule on the stretches where it has a value, with its asymptotes if asked.
    b.ctor(
        "Plot",
        VMOBJECT,
        vec![
            req("function", Type::Fun(Base::Real, Base::Real)),
            list("on", VEC),
            kw("step", REAL),
            kw("asymptotes", BOOL),
        ],
    );

    b.ctor(
        "ImageMobject",
        ROOT_KIND,
        vec![req("filename_or_array", STR)],
    );
}

fn methods(b: &mut Builder) {
    let m = ROOT_KIND;
    b.method("shift", m, vec![rest("vectors", VEC)]);
    b.method(
        "move_to",
        m,
        vec![req("point_or_mobject", anchor()), kw("aligned_edge", VEC)],
    );
    b.method(
        "next_to",
        m,
        vec![
            req("mobject_or_point", anchor()),
            pos("direction", VEC),
            kw("buff", REAL),
            kw("aligned_edge", VEC),
        ],
    );
    b.method("to_edge", m, vec![pos("edge", VEC), kw("buff", REAL)]);
    b.method("to_corner", m, vec![pos("corner", VEC), kw("buff", REAL)]);
    b.method(
        "align_to",
        m,
        vec![req("mobject_or_point", anchor()), pos("direction", VEC)],
    );
    let about = || vec![kw("about_point", VEC), kw("about_edge", VEC)];
    b.method(
        "scale",
        m,
        [vec![req("scale_factor", REAL)], about()].concat(),
    );
    b.method(
        "rotate",
        m,
        [vec![req("angle", REAL), kw("axis", VEC)], about()].concat(),
    );
    b.method("flip", m, vec![pos("axis", VEC)]);
    b.method("stretch", m, vec![req("factor", REAL), req("dim", INT)]);
    b.method("set_x", m, vec![req("x", REAL)]);
    b.method("set_y", m, vec![req("y", REAL)]);
    b.method("scale_to_fit_width", m, vec![req("width", REAL)]);
    b.method("scale_to_fit_height", m, vec![req("height", REAL)]);
    b.method(
        "arrange",
        m,
        vec![pos("direction", VEC), kw("buff", REAL), kw("center", BOOL)],
    );
    b.method("set_color", m, vec![req("color", COLOR)]);
    b.method("set_opacity", VMOBJECT, vec![req("opacity", REAL)]);
    b.method(
        "set_fill",
        VMOBJECT,
        vec![kw("color", COLOR), kw("opacity", REAL)],
    );
    b.method(
        "set_stroke",
        VMOBJECT,
        vec![kw("color", COLOR), kw("width", REAL), kw("opacity", REAL)],
    );
    b.method("set_value", "ValueTracker", vec![req("value", REAL)]);
    b.method("set_value", "DecimalNumber", vec![req("number", REAL)]);
    b.method("become", m, vec![req("mobject", mob(ROOT_KIND))]);
    b.sig.methods.get_mut("become").unwrap()[0].reshapes = true;
}

fn queries(b: &mut Builder) {
    let m = ROOT_KIND;
    for name in [
        "get_center",
        "get_top",
        "get_bottom",
        "get_left",
        "get_right",
        "get_start",
        "get_end",
    ] {
        b.query(name, m, vec![], Base::Vec);
    }
    b.query("get_corner", m, vec![req("direction", VEC)], Base::Vec);
    b.query("get_x", m, vec![], Base::Real);
    b.query("get_y", m, vec![], Base::Real);
    for name in ["width", "height"] {
        b.query(name, m, vec![], Base::Real);
        b.sig.queries.get_mut(name).unwrap()[0].property = true;
    }
    b.query(
        "point_from_proportion",
        VMOBJECT,
        vec![req("alpha", REAL)],
        Base::Vec,
    );
    b.query("get_value", "ValueTracker", vec![], Base::Real);
    b.query("get_value", "DecimalNumber", vec![], Base::Real);
    b.query(
        "c2p",
        "Axes",
        vec![req("x", REAL), req("y", REAL)],
        Base::Vec,
    );
    b.query("n2p", "NumberLine", vec![req("number", REAL)], Base::Vec);
}

fn animations(b: &mut Builder) {
    use Role::{Consume, Flash, Intro, Mut, Outro, Use};
    let m = ROOT_KIND;
    let v = VMOBJECT;

    let fade = || {
        vec![
            kw("shift", VEC),
            kw("target_position", anchor()),
            kw("scale", REAL),
        ]
    };
    b.anim("FadeIn", vec![obj("mobject", Intro, m)], fade());
    b.anim("FadeOut", vec![obj("mobject", Outro, m)], fade());

    b.anim("Create", vec![obj("mobject", Intro, v)], vec![]).lag = 1.0;
    b.anim("Uncreate", vec![obj("mobject", Consume, v)], vec![])
        .lag = 1.0;
    let draw = b.anim(
        "DrawBorderThenFill",
        vec![obj("vmobject", Intro, v)],
        vec![],
    );
    (draw.run_time, draw.rate) = (2.0, "double_smooth".into());
    b.anim("Write", vec![obj("vmobject", Intro, v)], vec![])
        .rate = "linear".into();
    b.anim("Unwrite", vec![obj("vmobject", Consume, v)], vec![])
        .rate = "linear".into();
    b.anim(
        "AddTextLetterByLetter",
        vec![obj("text", Intro, "Text")],
        vec![],
    )
    .rate = "linear".into();

    let tint = || vec![kw("point_color", COLOR)];
    b.anim("GrowFromCenter", vec![obj("mobject", Intro, m)], tint());
    b.anim(
        "GrowFromPoint",
        vec![obj("mobject", Intro, m)],
        [vec![req("point", VEC)], tint()].concat(),
    );
    b.anim(
        "GrowFromEdge",
        vec![obj("mobject", Intro, m)],
        [vec![req("edge", VEC)], tint()].concat(),
    );
    b.anim("GrowArrow", vec![obj("arrow", Intro, "Arrow")], tint());
    b.anim(
        "SpinInFromNothing",
        vec![obj("mobject", Intro, m)],
        [vec![kw("angle", REAL)], tint()].concat(),
    );
    b.anim(
        "ShowPassingFlash",
        vec![obj("mobject", Flash, v)],
        vec![kw("time_width", REAL)],
    );

    let arc = || vec![kw("path_arc", REAL)];
    b.anim(
        "Transform",
        vec![obj("mobject", Mut, m), obj("target_mobject", Use, m)],
        arc(),
    )
    .weakens = true;
    b.anim(
        "ReplacementTransform",
        vec![obj("mobject", Consume, m), obj("target_mobject", Intro, m)],
        arc(),
    );
    b.anim(
        "TransformFromCopy",
        vec![obj("mobject", Use, m), obj("target_mobject", Intro, m)],
        arc(),
    );
    b.anim(
        "FadeTransform",
        vec![obj("mobject", Outro, m), obj("target_mobject", Intro, m)],
        vec![kw("stretch", BOOL), kw("dim_to_match", INT)],
    );
    let matching = || {
        vec![
            kw("transform_mismatches", BOOL),
            kw("fade_transform_mismatches", BOOL),
        ]
    };
    // Composites of fades and transforms: the rate of an AnimationGroup is linear.
    b.anim(
        "TransformMatchingShapes",
        vec![obj("mobject", Outro, m), obj("target_mobject", Intro, m)],
        matching(),
    )
    .rate = "linear".into();
    b.anim(
        "TransformMatchingTex",
        vec![
            obj("mobject", Outro, "MathTex"),
            obj("target_mobject", Intro, "MathTex"),
        ],
        matching(),
    )
    .rate = "linear".into();

    b.anim(
        "Indicate",
        vec![obj("mobject", Mut, m)],
        vec![kw("scale_factor", REAL), kw("color", COLOR)],
    )
    .rate = "there_and_back".into();
    b.anim(
        "Wiggle",
        vec![obj("mobject", Mut, m)],
        vec![
            kw("scale_value", REAL),
            kw("rotation_angle", REAL),
            kw("n_wiggles", INT),
        ],
    )
    .run_time = 2.0;
    b.anim(
        "ApplyWave",
        vec![obj("mobject", Mut, m)],
        vec![kw("direction", VEC), kw("amplitude", REAL)],
    )
    .run_time = 2.0;
    // Draws a frame around the object and leaves the object itself untouched. Inside it is
    // a Succession, hence the linear rate and the lag of one.
    let circumscribe = b.anim(
        "Circumscribe",
        vec![obj("mobject", Use, m)],
        vec![
            kw("fade_in", BOOL),
            kw("fade_out", BOOL),
            kw("time_width", REAL),
            kw("buff", REAL),
            kw("color", COLOR),
            kw("stroke_width", REAL),
        ],
    );
    (circumscribe.rate, circumscribe.lag) = ("linear".into(), 1.0);
    b.anim(
        "Flash",
        vec![],
        vec![
            req("point", anchor()),
            kw("line_length", REAL),
            kw("num_lines", INT),
            kw("flash_radius", REAL),
            kw("color", COLOR),
        ],
    )
    .rate = "linear".into();
    b.anim(
        "FocusOn",
        vec![],
        vec![
            req("focus_point", anchor()),
            kw("opacity", REAL),
            kw("color", COLOR),
        ],
    )
    .run_time = 2.0;

    b.anim(
        "Rotate",
        vec![obj("mobject", Mut, m)],
        vec![
            kw("angle", REAL),
            kw("axis", VEC),
            kw("about_point", VEC),
            kw("about_edge", VEC),
        ],
    );
    b.anim(
        "MoveAlongPath",
        vec![obj("mobject", Mut, m), obj("path", Use, v)],
        vec![],
    );
    b.anim(
        "ScaleInPlace",
        vec![obj("mobject", Mut, m)],
        vec![req("scale_factor", REAL)],
    );
    b.anim(
        "FadeToColor",
        vec![obj("mobject", Mut, m)],
        vec![req("color", COLOR)],
    );
}

fn rates(b: &mut Builder) {
    // Exported by `from manim import *`.
    for name in [
        "linear",
        "smooth",
        "smoothstep",
        "smootherstep",
        "smoothererstep",
        "rush_into",
        "rush_from",
        "slow_into",
        "double_smooth",
        "there_and_back",
        "there_and_back_with_pause",
        "running_start",
        "wiggle",
        "lingering",
        "exponential_decay",
    ] {
        b.sig.rates.insert(name.into(), name.to_owned());
    }
    // Only reachable through the module.
    for family in [
        "sine", "quad", "cubic", "quart", "quint", "expo", "circ", "back", "elastic", "bounce",
    ] {
        for shape in ["in", "out", "in_out"] {
            let name = format!("ease_{shape}_{family}");
            b.sig
                .rates
                .insert(name.as_str().into(), format!("rate_functions.{name}"));
        }
    }
}

fn functions(b: &mut Builder) {
    use Base::{Bool, Int, Real, Vec};
    let infix = |op: &str| PyForm::Infix(op.to_owned());
    let call = |f: &str| PyForm::Call(f.to_owned());

    // Overloads are tried in order, so the integer forms come first.
    for op in ["+", "-", "*"] {
        b.function(op, &[Int, Int], Int, infix(op));
        b.function(op, &[Real, Real], Real, infix(op));
    }
    b.function("/", &[Real, Real], Real, infix("/"));
    b.function("+", &[Vec, Vec], Vec, infix("+"));
    b.function("-", &[Vec, Vec], Vec, infix("-"));
    b.function("*", &[Real, Vec], Vec, infix("*"));
    b.function("*", &[Vec, Real], Vec, infix("*"));
    b.function("/", &[Vec, Real], Vec, infix("/"));
    b.function("neg", &[Int], Int, PyForm::Prefix("-".to_owned()));
    b.function("neg", &[Real], Real, PyForm::Prefix("-".to_owned()));
    b.function("neg", &[Vec], Vec, PyForm::Prefix("-".to_owned()));

    for op in ["<", "<=", ">", ">=", "==", "!="] {
        b.function(op, &[Real, Real], Bool, infix(op));
    }
    b.function("and", &[Bool, Bool], Bool, infix("and"));
    b.function("or", &[Bool, Bool], Bool, infix("or"));
    b.function("not", &[Bool], Bool, PyForm::Prefix("not ".to_owned()));

    for f in ["sin", "cos", "tan", "exp", "log", "sqrt"] {
        b.function(f, &[Real], Real, call(&format!("np.{f}")));
    }
    b.function("abs", &[Real], Real, call("abs"));
    b.function("pow", &[Real, Real], Real, infix("**"));
    b.function("min", &[Real, Real], Real, call("min"));
    b.function("max", &[Real, Real], Real, call("max"));
    b.function("norm", &[Vec], Real, call("np.linalg.norm"));
    for (i, axis) in ["x", "y", "z"].into_iter().enumerate() {
        b.function(axis, &[Vec], Real, PyForm::Index(i));
    }
}

fn constants(b: &mut Builder) {
    let vectors: [(&str, [f64; 3]); 11] = [
        ("ORIGIN", [0.0, 0.0, 0.0]),
        ("UP", [0.0, 1.0, 0.0]),
        ("DOWN", [0.0, -1.0, 0.0]),
        ("RIGHT", [1.0, 0.0, 0.0]),
        ("LEFT", [-1.0, 0.0, 0.0]),
        ("OUT", [0.0, 0.0, 1.0]),
        ("IN", [0.0, 0.0, -1.0]),
        ("UL", [-1.0, 1.0, 0.0]),
        ("UR", [1.0, 1.0, 0.0]),
        ("DL", [-1.0, -1.0, 0.0]),
        ("DR", [1.0, -1.0, 0.0]),
    ];
    for (name, value) in vectors {
        b.sig.consts.insert(name.into(), Constant::Vec(value));
    }
    for (name, value) in [("PI", PI), ("TAU", TAU), ("DEGREES", TAU / 360.0)] {
        b.sig.consts.insert(name.into(), Constant::Real(value));
    }
    for (name, hex) in COLORS {
        let color = Rgba::parse(hex).expect("generated colour table holds valid hex");
        b.sig.consts.insert((*name).into(), Constant::Color(color));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_form_a_tree_rooted_at_mobject() {
        let sig = Signature::builtin();
        for (name, kind) in &sig.kinds {
            assert!(
                sig.is_subkind(name, ROOT_KIND),
                "{name} does not reach the root"
            );
            if let Some(parent) = &kind.parent {
                assert!(
                    sig.kinds.contains_key(parent),
                    "{name} has unknown parent {parent}"
                );
            }
        }
        assert!(sig.is_subkind("Square", "Polygon"));
        assert!(!sig.is_subkind("Triangle", "Polygon"));
        assert_eq!(&*sig.join("Circle", "Square"), VMOBJECT);
        assert_eq!(&*sig.join("Circle", "ImageMobject"), ROOT_KIND);
        assert_eq!(&*sig.elem("MathTex"), VMOBJECT);
        assert_eq!(&*sig.elem(GROUP_KIND), ROOT_KIND);
    }

    #[test]
    fn every_reference_resolves() {
        let sig = Signature::builtin();
        let known = |kind: &str| assert!(sig.kinds.contains_key(kind), "unknown kind {kind}");
        sig.ctors.values().for_each(|c| known(&c.kind));
        sig.methods
            .values()
            .flatten()
            .for_each(|m| known(&m.receiver));
        sig.queries
            .values()
            .flatten()
            .for_each(|q| known(&q.receiver));
        for (name, anim) in &sig.anims {
            assert!(
                sig.rates.contains_key(&anim.rate),
                "{name} has unknown rate {}",
                anim.rate
            );
            for p in &anim.params {
                if let AnimParamType::Object { kind, .. } = &p.ty {
                    known(kind);
                }
            }
        }
    }

    #[test]
    fn parameters_are_inherited_and_overloads_resolve() {
        let sig = Signature::builtin();
        let names: Vec<&str> = sig
            .ctor_params(&sig.ctors["Circle"])
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(
            names,
            [
                "radius",
                "fill_color",
                "fill_opacity",
                "stroke_color",
                "stroke_opacity",
                "stroke_width",
                "color"
            ]
        );
        assert_eq!(
            &*sig.method("Integer", "set_value").unwrap().params[0].name,
            "number"
        );
        assert_eq!(
            &*sig.method("ValueTracker", "set_value").unwrap().params[0].name,
            "value"
        );
        assert!(sig.method("ImageMobject", "set_fill").is_none());
        assert!(sig.query("NumberPlane", "c2p").is_some());
    }
}
