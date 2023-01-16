//! The data types and interaction logic for the calculator.

#![warn(clippy::all)]
#![warn(missing_copy_implementations, missing_docs, rust_2018_idioms)]
#![deny(unsafe_op_in_unsafe_fn, missing_debug_implementations)]
#![cfg_attr(not(debug_assertions), deny(clippy::todo))]

use std::{collections::HashMap, rc::Rc};

mod stack;
pub use stack::*;

mod recipe;
pub use recipe::*;

/// The actual calculator.
#[derive(Clone, Debug)]
pub struct Calculator {
    recipes: HashMap<String, Rc<Recipe>>,
    target: Stack,
    materials: HashMap<String, Count>,
    steps: Vec<(Rc<Recipe>, Count)>,
}

impl Calculator {
    /// Creates a calculator that knows about the given recipes.
    pub fn new(recipes: HashMap<String, Recipe>) -> Self {
        Self {
            recipes: recipes
                .into_iter()
                .map(|(output, recipe)| (output, Rc::new(recipe)))
                .collect(),
            target: Stack::new("Air", 1),
            materials: HashMap::new(),
            steps: vec![],
        }
    }

    /// Gets the calculator's current target.
    pub fn target(&self) -> &Stack {
        &self.target
    }

    /// Sets the recipe for creating [`recipe.result()`] [`.item()`].
    ///
    /// [`recipe.result()`]: /struct.Recipe.html#method.result
    /// [`.item()`]: /struct.Stack.html#method.item
    pub fn set_recipe(&mut self, _recipe: Recipe) {
        todo!("Set recipe and recalculate steps")
    }

    /// Sets the target for the calculator.
    pub fn set_target(&mut self, target: Stack) {
        self.target = target;
        todo!("Recalculate steps")
    }

    /// Gets the steps to convert the available materials into [`self.target()`].
    ///
    /// [`self.target()`]: #method.target
    pub fn get_steps(&self) -> impl Iterator<Item = (&Recipe, Count)> + '_ {
        self.steps
            .iter()
            .map(|&(ref recipe, count)| (Rc::as_ref(recipe), count))
    }
}

mod util;
