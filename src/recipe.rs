use nom::{
    branch, bytes::complete as bytes, character::complete as character, combinator, multi,
    sequence, IResult, Parser,
};

use crate::Stack;

/// A known way to produce a stack from a set of other stacks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Recipe {
    result: Stack,
    method: String,
    ingredients: Vec<Stack>,
}

impl Recipe {
    /// Creates a new recipe representing the ability to convert `ingredients` into `result` using
    /// `method`.
    pub fn new(result: Stack, method: impl Into<String>, ingredients: Vec<Stack>) -> Self {
        Self {
            result,
            method: method.into(),
            ingredients,
        }
    }
}

impl Recipe {
    /// The stack that is produced by executing this recipe once.
    pub fn result(&self) -> &Stack {
        &self.result
    }

    /// The method by which the ingredients are turned into the result.
    pub fn method(&self) -> &str {
        &self.method
    }

    /// The stacks that are required to execute this recipe once.
    pub fn ingredients(&self) -> &[Stack] {
        &self.ingredients
    }
}

impl Recipe {
    pub(crate) fn nom_parse(
        default_method: &str,
    ) -> impl Parser<&str, Recipe, nom::error::Error<&str>> + '_ {
        RecipeParser { default_method }
    }

    /// Parses a list of recipes separated by a blank line.
    pub fn parse_recipes(
        default_method: &str,
    ) -> impl Parser<&str, Vec<Recipe>, nom::error::Error<&str>> + '_ {
        multi::many0(Recipe::nom_parse(default_method))
    }
}

struct RecipeParser<'d> {
    default_method: &'d str,
}

impl<'i, 'd> Parser<&'i str, Recipe, nom::error::Error<&'i str>> for RecipeParser<'d>
where
    'd: 'i,
{
    fn parse(&mut self, s: &'i str) -> IResult<&'i str, Recipe> {
        let result_and_method = sequence::pair(
            Stack::nom_parse,
            sequence::terminated(
                combinator::opt(sequence::delimited(
                    bytes::tag(" ("),
                    combinator::recognize(multi::many1(character::none_of(")"))),
                    bytes::tag(")"),
                )),
                bytes::tag(":"),
            ),
        );
        let single_ingredient = combinator::map(
            sequence::preceded(bytes::tag(" "), Stack::nom_parse),
            |ingredient| vec![ingredient],
        );
        let multiple_ingredients = multi::many1(sequence::preceded(
            sequence::pair(character::line_ending, character::space1),
            Stack::nom_parse,
        ));
        combinator::map(
            sequence::pair(
                result_and_method,
                sequence::terminated(
                    branch::alt((single_ingredient, multiple_ingredients)),
                    character::line_ending,
                ),
            ),
            |((result, method), ingredients)| Recipe {
                result,
                method: method.unwrap_or(self.default_method).to_string(),
                ingredients,
            },
        )(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE_LINE_NO_METHOD: &str = "Oak Wood Planks (4): Oak Log (1)\n";
    const ONE_LINE_WITH_METHOD: &str = "Charcoal (1) (Furnace): Oak Log (1)\n";
    const MULTI_LINE: &str = "Wooden Shovel (1):\n Oak Wood Planks (1)\n Stick (2)\n";

    #[test]
    fn parse_one_line_recipe_implicit_method() {
        let expected = (
            "",
            Recipe {
                result: Stack::new("Oak Wood Planks", 4),
                method: "Crafting Table".to_string(),
                ingredients: vec![Stack::new("Oak Log", 1)],
            },
        );
        let actual = Recipe::nom_parse("Crafting Table")
            .parse(ONE_LINE_NO_METHOD)
            .unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_one_line_recipe_explicit_method() {
        let expected = (
            "",
            Recipe {
                result: Stack::new("Charcoal", 1),
                method: "Furnace".to_string(),
                ingredients: vec![Stack::new("Oak Log", 1)],
            },
        );
        let actual = Recipe::nom_parse("Crafting Table")
            .parse(ONE_LINE_WITH_METHOD)
            .unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_multi_line_recipe() {
        let expected = (
            "",
            Recipe {
                result: Stack::new("Wooden Shovel", 1),
                method: "Crafting Table".to_string(),
                ingredients: vec![Stack::new("Oak Wood Planks", 1), Stack::new("Stick", 2)],
            },
        );
        let actual = Recipe::nom_parse("Crafting Table")
            .parse(MULTI_LINE)
            .unwrap();
        assert_eq!(expected, actual);
    }
}
