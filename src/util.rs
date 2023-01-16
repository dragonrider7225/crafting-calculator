use nom::{
    bytes::complete as bytes, character::complete as character, combinator, multi, sequence,
    IResult,
};

pub(crate) fn read_usize(s: &str) -> IResult<&str, usize> {
    combinator::map(
        combinator::recognize(multi::many1(sequence::terminated(
            character::one_of("0123456789"),
            multi::many0(bytes::tag("_")),
        ))),
        |s: &str| {
            s.bytes().fold(0, |acc, c| {
                if c == b'_' {
                    acc
                } else {
                    acc * 10 + (c - b'0') as usize
                }
            })
        },
    )(s)
}
