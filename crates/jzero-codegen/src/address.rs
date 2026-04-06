/// A memory region in the generated program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Region {
    /// Local stack slot, offset relative to frame pointer.
    Loc,
    /// Statically allocated global variable.
    Global,
    /// Read-only string literal pool.
    Strings,
    /// Code label (offset is the unique label id).
    Lab,
    /// Heap-relative offset (instance fields).
    Class,
    /// Immediate value — offset *is* the value, not an address.
    Imm,
    /// Implicit self/this pointer.
    Self_,
}

impl std::fmt::Display for Region {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Region::Loc     => write!(f, "loc"),
            Region::Global  => write!(f, "global"),
            Region::Strings => write!(f, "strings"),
            Region::Lab     => write!(f, "L"),
            Region::Class   => write!(f, "class"),
            Region::Imm     => write!(f, "imm"),
            Region::Self_   => write!(f, "self"),
        }
    }
}

/// An address in the generated program.
///
/// Either a region+offset pair (the common case), or a bare symbolic name
/// used for emitting mangled method names like `PrintStream__println`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Address {
    /// Region + integer offset.
    Regional { region: Region, offset: i64 },
    /// A symbolic method name, printed as a bare identifier.
    Symbol(String),
}

impl Address {
    pub fn new(region: Region, offset: i64) -> Self {
        Address::Regional { region, offset }
    }
    pub fn imm(value: i64)      -> Self { Address::new(Region::Imm,     value) }
    pub fn loc(offset: i64)     -> Self { Address::new(Region::Loc,     offset) }
    pub fn global(offset: i64)  -> Self { Address::new(Region::Global,  offset) }
    pub fn strings(offset: i64) -> Self { Address::new(Region::Strings, offset) }
    pub fn lab(id: i64)         -> Self { Address::new(Region::Lab,     id) }
    pub fn symbol(name: &str)   -> Self { Address::Symbol(name.to_string()) }
    pub fn self_ptr()           -> Self { Address::new(Region::Self_,   0) }
}

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Address::Regional { region: Region::Lab, offset } =>
                write!(f, "L{}", offset),
            Address::Regional { region: Region::Self_, .. } =>
                write!(f, "self"),
            Address::Regional { region, offset } =>
                write!(f, "{}:{}", region, offset),
            Address::Symbol(name) =>
                write!(f, "{}", name),
        }
    }
}