use serde::{Deserialize, Serialize};

macro_rules! id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(
            Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Default, Serialize,
            Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub u64);
    };
}

id!(
    /// Identity of an AST node; stable across edits, unique within a deck.
    NodeId
);
id!(
    /// Identity of a variable. The display name lives in `Deck::names`.
    VarId
);
id!(SlideId);
