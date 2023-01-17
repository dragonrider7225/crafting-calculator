use nom::{
    bytes::complete as bytes, character::complete as character, combinator as comb, multi,
    sequence, IResult,
};

/// The number of items in a stack.
pub type Count = usize;

/// A stack of some number of all the same item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stack {
    name: String,
    count: Count,
}

impl Stack {
    /// Makes a new stack of `name` containing `count` items.
    pub fn new(name: impl Into<String>, count: Count) -> Self {
        Self {
            name: name.into(),
            count,
        }
    }
}

impl Stack {
    /// The item in the stack.
    pub fn item(&self) -> &str {
        &self.name
    }

    /// The number of items in the stack.
    pub fn count(&self) -> Count {
        self.count
    }
}

impl Stack {
    pub(crate) fn nom_parse(s: &str) -> IResult<&str, Self> {
        comb::map(
            sequence::pair(
                comb::recognize(multi::many1(character::none_of("("))),
                sequence::delimited(bytes::tag("("), crate::util::read_usize, bytes::tag(")")),
            ),
            |(name, count)| Self {
                name: name.trim().to_string(),
                count,
            },
        )(s)
    }
}
