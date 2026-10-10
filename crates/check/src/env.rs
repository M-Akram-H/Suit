//! Contexts `Γ` and presence trees `T` (spec §5.1).

use std::collections::BTreeMap;

use suit_core::{Place, Presence, Sym, VarId};
use suit_sig::Base;

/// Presence of a place: uniform, or `Mixed` when its parts disagree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Is(Presence),
    Mixed,
}

/// What is known about an object's children.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Shape {
    /// Unknown until the object is built (how many glyphs has a formula?).
    Opaque,
    /// A group: the members, with the kinds they were grouped as.
    Group(Vec<Member>),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Member {
    pub kind: Sym,
    pub shape: Shape,
}

static OPAQUE: Shape = Shape::Opaque;

impl Shape {
    /// Shape of child `index`; `None` when a group has no such member.
    pub fn child(&self, index: u32) -> Option<&Shape> {
        match self {
            Shape::Opaque => Some(&OPAQUE),
            Shape::Group(members) => members.get(index as usize).map(|m| &m.shape),
        }
    }
}

/// Presence tree. Children not listed in a `Split` share `rest`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Tree {
    Uniform(Presence),
    Split {
        rest: Presence,
        children: BTreeMap<u32, Tree>,
    },
}

impl Tree {
    pub fn state(&self, shape: &Shape) -> State {
        let (rest, children) = match self {
            Tree::Uniform(presence) => return State::Is(*presence),
            Tree::Split { rest, children } => (rest, children),
        };
        let mut common = None;
        for (&index, child) in children {
            match child.state(shape.child(index).unwrap_or(&OPAQUE)) {
                State::Is(p) if common.is_none_or(|c| c == p) => common = Some(p),
                _ => return State::Mixed,
            }
        }
        match (common, shape) {
            (None, _) => State::Is(*rest),
            (Some(p), _) if p == *rest => State::Is(p),
            // Every member is listed, so `rest` describes nothing.
            (Some(p), Shape::Group(members)) if children.len() == members.len() => State::Is(p),
            _ => State::Mixed,
        }
    }

    /// The subtree at `path`.
    pub fn at(&self, path: &[u32]) -> Tree {
        let mut tree = self;
        for index in path {
            match tree {
                Tree::Uniform(_) => break,
                Tree::Split { rest, children } => match children.get(index) {
                    Some(child) => tree = child,
                    None => return Tree::Uniform(*rest),
                },
            }
        }
        tree.clone()
    }

    /// `Γ[p ↦ π]`: overwrite the subtree at `path` and collapse what became uniform.
    pub fn set(&mut self, shape: &Shape, path: &[u32], presence: Presence) {
        let Some((&first, deeper)) = path.split_first() else {
            *self = Tree::Uniform(presence);
            return;
        };
        if let Tree::Uniform(current) = *self {
            if current == presence {
                return;
            }
            *self = Tree::Split {
                rest: current,
                children: BTreeMap::new(),
            };
        }
        let Tree::Split { rest, children } = self else {
            unreachable!()
        };
        let child_shape = shape.child(first).unwrap_or(&OPAQUE);
        children
            .entry(first)
            .or_insert(Tree::Uniform(*rest))
            .set(child_shape, deeper, presence);
        if let State::Is(uniform) = self.state(shape) {
            *self = Tree::Uniform(uniform);
        }
    }

    /// Whether any part is on scene.
    pub fn any_on(&self) -> bool {
        match self {
            Tree::Uniform(presence) => *presence == Presence::On,
            Tree::Split { rest, children } => {
                *rest == Presence::On || children.values().any(Tree::any_on)
            }
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Object {
    pub kind: Sym,
    pub shape: Shape,
    pub tree: Tree,
    /// Places that took on another object's geometry, with their weakened kind. Nothing
    /// below such a place can be indexed any more (spec §5.4).
    pub reshaped: BTreeMap<Vec<u32>, Sym>,
}

#[derive(Clone, Debug)]
pub(crate) enum Entry {
    Value(Base),
    Object(Object),
    /// `x ≔ p`: the variable was moved into a group and now names a place inside it.
    Alias(Place),
}

/// An attached updater: the place it sits on and the places its arguments read each frame.
#[derive(Clone, Debug)]
pub(crate) struct Attached {
    pub target: Place,
    pub reads: Vec<Place>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Env {
    pub vars: BTreeMap<VarId, Entry>,
    pub updaters: Vec<Attached>,
}

impl Env {
    /// Follows aliases until the root is an object. `None` for values and unknown variables.
    pub fn resolve(&self, place: &Place) -> Option<Place> {
        let mut place = place.clone();
        loop {
            match self.vars.get(&place.root)? {
                Entry::Object(_) => return Some(place),
                Entry::Value(_) => return None,
                Entry::Alias(target) => {
                    let mut path = target.path.clone();
                    path.extend_from_slice(&place.path);
                    place = Place {
                        root: target.root,
                        path,
                    };
                }
            }
        }
    }

    pub fn object(&self, root: VarId) -> Option<&Object> {
        match self.vars.get(&root)? {
            Entry::Object(object) => Some(object),
            _ => None,
        }
    }

    /// Sets the presence of a resolved place.
    pub fn set(&mut self, place: &Place, presence: Presence) {
        if let Some(Entry::Object(object)) = self.vars.get_mut(&place.root) {
            object.tree.set(&object.shape, &place.path, presence);
        }
    }

    /// Whether some updater sits on something at least partly on scene.
    pub fn updaters_live(&self) -> bool {
        self.updaters.iter().any(|updater| {
            self.resolve(&updater.target)
                .and_then(|p| Some(self.object(p.root)?.tree.at(&p.path).any_on()))
                .unwrap_or(false)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Presence::{Off, On};

    fn group(n: usize) -> Shape {
        Shape::Group(vec![
            Member {
                kind: "Circle".into(),
                shape: Shape::Opaque
            };
            n
        ])
    }

    #[test]
    fn parts_of_an_opaque_object() {
        let mut tree = Tree::Uniform(On);
        tree.set(&Shape::Opaque, &[0], Off);
        assert_eq!(tree.state(&Shape::Opaque), State::Mixed);
        assert_eq!(tree.at(&[0]).state(&Shape::Opaque), State::Is(Off));
        assert_eq!(tree.at(&[5, 1]).state(&Shape::Opaque), State::Is(On));
        tree.set(&Shape::Opaque, &[0], On);
        assert_eq!(tree, Tree::Uniform(On));
    }

    #[test]
    fn a_group_with_every_member_changed_is_uniform_again() {
        let shape = group(2);
        let mut tree = Tree::Uniform(Off);
        tree.set(&shape, &[0], On);
        assert_eq!(tree.state(&shape), State::Mixed);
        assert!(tree.any_on());
        tree.set(&shape, &[1], On);
        assert_eq!(tree, Tree::Uniform(On));
    }

    #[test]
    fn aliases_resolve_into_the_group() {
        let (x, g, outer) = (VarId(1), VarId(2), VarId(3));
        let mut env = Env::default();
        let object = Object {
            kind: "Group".into(),
            shape: group(1),
            tree: Tree::Uniform(Off),
            reshaped: BTreeMap::new(),
        };
        env.vars.insert(outer, Entry::Object(object));
        env.vars.insert(
            g,
            Entry::Alias(Place {
                root: outer,
                path: vec![0],
            }),
        );
        env.vars.insert(
            x,
            Entry::Alias(Place {
                root: g,
                path: vec![1],
            }),
        );
        let resolved = env
            .resolve(&Place {
                root: x,
                path: vec![4],
            })
            .unwrap();
        assert_eq!(
            resolved,
            Place {
                root: outer,
                path: vec![0, 1, 4]
            }
        );
    }
}
