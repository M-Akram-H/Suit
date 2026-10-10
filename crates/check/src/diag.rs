use suit_core::NodeId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// A rule of spec §5 is violated; the deck cannot be exported.
    Error,
    /// Legal, but probably not what was meant (spec §5.8).
    Warning,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Stable identifier of the rule, e.g. `presence` or `overlap`.
    pub code: &'static str,
    /// Index of the slide it was found on.
    pub slide: usize,
    /// The command or animation at fault; `None` for the slide as a whole.
    pub node: Option<NodeId>,
    pub message: String,
    /// What to do about it.
    pub help: Option<String>,
}
