use crate::{Anim, Arg, Args, Command, Node, NodeId, Object, Updater};

impl Node<Command> {
    /// The ids of this command and of every node nested in it.
    pub fn ids(&self) -> Vec<NodeId> {
        let mut out = vec![self.id];
        match &self.item {
            Command::LetObject { value, .. } => object(value, &mut out),
            Command::Apply { call, .. } => args(&call.args, &mut out),
            Command::Play { anims } => anims.iter().for_each(|a| anim(a, &mut out)),
            Command::Attach { updater, .. } => match updater {
                Updater::Always(call) | Updater::Every { call, .. } => args(&call.args, &mut out),
            },
            Command::LetValue { .. }
            | Command::Add { .. }
            | Command::Remove { .. }
            | Command::Wait { .. }
            | Command::Detach { .. }
            | Command::Raw(_) => {}
        }
        out
    }
}

fn object(node: &Node<Object>, out: &mut Vec<NodeId>) {
    out.push(node.id);
    match &node.item {
        Object::New { args: given, .. } => args(given, out),
        Object::Chain { base, call } => {
            object(base, out);
            args(&call.args, out);
        }
        Object::Group { .. } | Object::Copy(_) => {}
    }
}

fn args(args: &Args, out: &mut Vec<NodeId>) {
    for (_, arg) in args {
        if let Arg::Object(inline) = arg {
            object(inline, out);
        }
    }
}

fn anim(node: &Node<Anim>, out: &mut Vec<NodeId>) {
    out.push(node.id);
    match &node.item {
        Anim::Prim { args: given, .. } => args(given, out),
        Anim::Animate { calls, .. } => calls.iter().for_each(|call| args(&call.args, out)),
        Anim::Par { items, .. } | Anim::Seq { items, .. } => {
            items.iter().for_each(|item| anim(item, out))
        }
    }
}
