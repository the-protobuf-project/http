//! Choosing which methods an interceptor applies to.

/// How a method is classified against the AIP standard methods.
///
/// Emitted by `protoc-gen-http` from the method's name and its
/// `google.api.http` rule. It is what makes a policy expressible in terms of
/// what a method *means* rather than what it is called.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MethodPattern {
    /// AIP-136: anything that is not a standard method. The default, because
    /// it is the honest answer for an unannotated method.
    Custom,
    /// AIP-131 Get.
    Get,
    /// AIP-132 List.
    List,
    /// AIP-133 Create.
    Create,
    /// AIP-134 Update.
    Update,
    /// AIP-135 Delete.
    Delete,
    /// AIP-164 Undelete.
    Undelete,
    /// AIP-231 `BatchGet`.
    BatchGet,
    /// AIP-233 `BatchCreate`.
    BatchCreate,
    /// AIP-234 `BatchUpdate`.
    BatchUpdate,
    /// AIP-235 `BatchDelete`.
    BatchDelete,
}

impl MethodPattern {
    /// Whether the pattern changes state.
    ///
    /// A custom method counts as mutating: it is the conservative reading, and
    /// a read-only custom method that wants otherwise can be named explicitly.
    #[must_use]
    pub const fn is_mutating(self) -> bool {
        !matches!(
            self,
            MethodPattern::Get | MethodPattern::List | MethodPattern::BatchGet
        )
    }

    /// Whether the pattern only reads.
    #[must_use]
    pub const fn is_read_only(self) -> bool {
        !self.is_mutating()
    }
}

/// Which methods an interceptor applies to.
///
/// Borrowed from `go-grpc-middleware`, and stronger here because the IR knows
/// what methods *mean*: `Selector::Mutating` covers every Create, Update,
/// Delete, and Undelete, so adding one later is covered automatically. A policy
/// written against a name prefix would silently miss it.
#[derive(Debug, Clone)]
pub enum Selector {
    /// Every method.
    All,
    /// Methods that change state.
    Mutating,
    /// Methods that only read.
    ReadOnly,
    /// One AIP pattern.
    Pattern(MethodPattern),
    /// Every method of one service, by fully-qualified name.
    Service(&'static str),
    /// One method, by fully-qualified name.
    Method(&'static str),
    /// Every method *except* those the inner selector matches.
    Not(Box<Selector>),
    /// Any of the inner selectors.
    Any(Vec<Selector>),
    /// All of the inner selectors.
    All_(Vec<Selector>),
}

impl Selector {
    /// Whether this selector covers a method.
    #[must_use]
    pub fn matches(&self, service: &str, method: &str, pattern: MethodPattern) -> bool {
        match self {
            Selector::All => true,
            Selector::Mutating => pattern.is_mutating(),
            Selector::ReadOnly => pattern.is_read_only(),
            Selector::Pattern(want) => *want == pattern,
            Selector::Service(name) => *name == service,
            Selector::Method(name) => *name == method,
            Selector::Not(inner) => !inner.matches(service, method, pattern),
            Selector::Any(inner) => inner.iter().any(|s| s.matches(service, method, pattern)),
            Selector::All_(inner) => inner.iter().all(|s| s.matches(service, method, pattern)),
        }
    }

    /// Negates the selector.
    ///
    /// Named `except` rather than `not` so it cannot be confused with
    /// [`std::ops::Not::not`], which has different semantics.
    #[must_use]
    pub fn except(self) -> Selector {
        Selector::Not(Box::new(self))
    }
}
