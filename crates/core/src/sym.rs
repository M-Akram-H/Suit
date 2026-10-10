use std::{borrow::Borrow, fmt, ops::Deref, sync::Arc};

use serde::{Deserialize, Serialize};

/// Key into the signature Σ: "Circle", "shift", "FadeIn", "smooth", "+", "UP".
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Sym(Arc<str>);

impl Sym {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Sym {
    fn from(s: &str) -> Self {
        Sym(Arc::from(s))
    }
}

impl From<String> for Sym {
    fn from(s: String) -> Self {
        Sym(Arc::from(s))
    }
}

impl Deref for Sym {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for Sym {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Sym {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for Sym {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.0, f)
    }
}
