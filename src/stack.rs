use std::{
    fmt::{self, Display, Formatter},
    ops::{Mul, MulAssign},
    str::FromStr,
};

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
        let mut name = name.into();
        let end_whitespace_len = name.trim_end().len();
        name.truncate(end_whitespace_len);
        let start_whitespace_len = name.len() - name.trim_start().len();
        name.replace_range(..start_whitespace_len, "");
        Self { name, count }
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

impl Display for Stack {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.item(), self.count())
    }
}

impl FromStr for Stack {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use nom::Finish;

        Self::nom_parse(s)
            .finish()
            .map(|(_, stack)| stack)
            .map_err(|e| format!("Couldn't parse stack: {e:?}"))
    }
}

impl Mul<Count> for Stack {
    type Output = Self;

    fn mul(mut self, rhs: Count) -> Self::Output {
        self *= rhs;
        self
    }
}

impl MulAssign<Count> for Stack {
    fn mul_assign(&mut self, rhs: Count) {
        self.count *= rhs;
    }
}

impl Mul<Count> for &'_ Stack {
    type Output = Stack;

    fn mul(self, rhs: Count) -> Self::Output {
        Stack {
            name: self.name.clone(),
            count: self.count * rhs,
        }
    }
}
