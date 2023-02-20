use std::{
    collections::{HashMap, HashSet},
    mem,
    rc::Rc,
};

use priority_queue::DoublePriorityQueue;

use crate::{Count, Recipe, Stack};

/// The actual calculator.
#[derive(Clone, Debug)]
pub struct Calculator {
    recipes: HashMap<String, Rc<Recipe>>,
    target: Stack,
    initial_materials: HashMap<String, Count>,
    materials: HashMap<String, Count>,
    crafted_materials: HashMap<String, Count>,
    steps: Vec<(Rc<Recipe>, Count)>,
}

impl Calculator {
    /// Creates a calculator that doesn't know about any recipes.
    pub fn new() -> Self {
        Self::with_recipes(HashMap::new())
    }

    /// Creates a calculator that knows about the given recipes.
    pub fn with_recipes(recipes: HashMap<String, Recipe>) -> Self {
        Self {
            recipes: recipes
                .into_iter()
                .map(|(output, recipe)| (output, Rc::new(recipe)))
                .collect(),
            target: Stack::new("Air", 1),
            initial_materials: Default::default(),
            materials: Default::default(),
            crafted_materials: Default::default(),
            steps: Default::default(),
        }
    }

    /// Gets the recipes that the calculator knows about.
    pub fn recipes(&self) -> impl Iterator<Item = &Recipe> + '_ {
        self.recipes.values().map(Rc::as_ref)
    }

    /// Gets the calculator's current target.
    pub fn target(&self) -> &Stack {
        &self.target
    }

    /// Adds the given stack to the set of resources that are already available and do not need to
    /// be crafted.
    pub fn add_resource(&mut self, resource: Stack) {
        match self.initial_materials.get_mut(resource.item()) {
            Some(count) => *count += resource.count(),
            None => {
                self.initial_materials
                    .insert(resource.item().to_string(), resource.count());
            }
        }
        self.calculate_steps();
    }

    fn calculate_steps(&mut self) {
        self.steps.clear();
        self.materials.clone_from(&self.initial_materials);
        self.crafted_materials.clear();
        let mut to_craft = HashMap::new();
        to_craft.insert(self.target.item(), self.target.count());
        let mut craft_order = DoublePriorityQueue::new();
        craft_order.push(self.target.item(), 0);
        while let Some((next_craft, _)) = craft_order.pop_min() {
            if let Some(mut count) = to_craft.remove(next_craft) {
                if let Some(available) = self.crafted_materials.get_mut(next_craft) {
                    let retrieved = (*available).min(count);
                    *available -= retrieved;
                    count -= retrieved;
                }
                if let Some(available) = self.materials.get_mut(next_craft) {
                    let retrieved = (*available).min(count);
                    if retrieved > 0 {
                        self.steps.push((
                            Rc::new(Recipe::new(
                                Stack::new(next_craft, 1),
                                "In Storage",
                                vec![Stack::new(next_craft, 1)],
                            )),
                            retrieved,
                        ));
                        *available -= retrieved;
                        count -= retrieved;
                    }
                }
                if count > 0 {
                    if let Some(recipe) = self.recipes.get(next_craft) {
                        let per_execution = recipe.result().count();
                        let repeats = (1..).find(|i| i * per_execution >= count).unwrap();
                        self.steps.push((Rc::clone(recipe), repeats));
                        let produced = per_execution * repeats;
                        if produced > count {
                            let excess = produced - count;
                            // We don't need to worry about overwriting an existing entry because
                            // that would require `*available > count` up above, which always makes
                            // `retrieved == count`.
                            self.crafted_materials
                                .insert(next_craft.to_string(), excess);
                        }
                        for ingredient in recipe.ingredients() {
                            let next_priority = craft_order
                                .peek_max()
                                .map(|(_, &priority)| priority + 1)
                                .unwrap_or_default();
                            if next_priority == usize::MAX {
                                // We've run out of space to keep increasing the priority, so we
                                // need to squish the remaining priorities down.
                                let to_craft = craft_order.into_ascending_sorted_vec();
                                craft_order = DoublePriorityQueue::new();
                                craft_order.extend(
                                    to_craft.into_iter().enumerate().map(|(idx, c)| (c, idx)),
                                );
                            }
                            craft_order.push_increase(ingredient.item(), next_priority);
                            *to_craft.entry(ingredient.item()).or_default() +=
                                ingredient.count() * repeats;
                        }
                    } else {
                        self.steps.push((
                            Rc::new(Recipe::new(
                                Stack::new(next_craft, 1),
                                "Raw Material",
                                vec![Stack::new(next_craft, 1)],
                            )),
                            count,
                        ));
                    }
                }
            }
        }
        debug_assert!(to_craft.is_empty());
        let mut checked_steps = vec![];
        let mut available_materials = HashSet::new();
        let mut from_storage = HashMap::new();
        let mut steps_to_check = mem::take(&mut self.steps);
        let mut tmp = vec![];
        // Separate out the raw materials
        {
            let mut raw_materials = HashMap::new();
            for (step, repeats) in steps_to_check.drain(..) {
                if step.method() != "Raw Material" {
                    tmp.push((step, repeats));
                    continue;
                }
                match raw_materials.get_mut(step.result().item()) {
                    Some((_, cached_repeats)) => *cached_repeats += repeats,
                    None => {
                        raw_materials.insert(step.result().item().to_string(), (step, repeats));
                    }
                }
            }
            checked_steps.reserve(raw_materials.len());
            for (result, action) in raw_materials {
                checked_steps.push(action);
                available_materials.insert(result);
            }
            steps_to_check.append(&mut tmp);
        }
        // Separate out the materials from storage
        {
            for (step, repeats) in steps_to_check.drain(..) {
                if step.method() != "In Storage" {
                    tmp.push((step, repeats));
                    continue;
                }
                match from_storage.get_mut(step.result().item()) {
                    Some((_, cached_repeats)) => *cached_repeats += repeats,
                    None => {
                        from_storage.insert(step.result().item().to_string(), (step, repeats));
                    }
                }
            }
            steps_to_check.append(&mut tmp);
        }
        // Build out the set of steps that can be taken using only the raw materials and the
        // things that have already been crafted.
        while !steps_to_check.is_empty() {
            let mut current_stage = HashMap::new();
            for (step, repeats) in steps_to_check.drain(..) {
                if !step.ingredients().iter().all(|stack| {
                    available_materials.contains(stack.item())
                        || from_storage
                            .get(stack.item())
                            .filter(|&&(ref recipe, rec_repeats)| {
                                recipe.result().count() * rec_repeats >= stack.count() * repeats
                            })
                            .is_some()
                }) {
                    tmp.push((step, repeats));
                    continue;
                }
                for stack in step.ingredients().iter() {
                    match from_storage.remove_entry(stack.item()) {
                        None => {}
                        Some((item, (recipe, rec_repeats))) => {
                            checked_steps.push((recipe, rec_repeats * repeats));
                            available_materials.insert(item);
                        }
                    }
                }
                match current_stage.get_mut(step.result().item()) {
                    Some((_, cached_repeats)) => *cached_repeats += repeats,
                    None => {
                        current_stage.insert(step.result().item().to_string(), (step, repeats));
                    }
                }
            }
            checked_steps.reserve(current_stage.len());
            for (result, action) in current_stage {
                checked_steps.push(action);
                available_materials.insert(result);
            }
            steps_to_check.append(&mut tmp);
        }
        self.steps = checked_steps;
    }

    /// Sets the recipe for creating [`recipe.result()`] [`.item()`].
    ///
    /// [`recipe.result()`]: /struct.Recipe.html#method.result
    /// [`.item()`]: /struct.Stack.html#method.item
    pub fn set_recipe(&mut self, recipe: Recipe) {
        self.add_recipes(vec![recipe]);
    }

    /// Sets the calculator to use the specified recipes for creating their results. If multiple
    /// recipes produce the same item, the later recipe overrides the earlier one(s).
    pub fn add_recipes(&mut self, recipes: Vec<Recipe>) {
        for recipe in recipes {
            let name = recipe.result().item();
            self.recipes.insert(name.to_string(), Rc::new(recipe));
        }
        self.calculate_steps();
    }

    /// Sets the target for the calculator.
    pub fn set_target(&mut self, target: Stack) {
        self.target = target;
        self.calculate_steps();
    }

    /// Gets the steps to convert the available materials into [`self.target()`].
    ///
    /// [`self.target()`]: #method.target
    pub fn steps(&self) -> impl Iterator<Item = (&Recipe, Count)> + '_ {
        self.steps
            .iter()
            .map(|&(ref recipe, count)| (Rc::as_ref(recipe), count))
    }
}

impl Default for Calculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculate_raw_material() {
        let expected = [(
            &Recipe::new(
                Stack::new("Oak Log", 1),
                "Raw Material",
                vec![Stack::new("Oak Log", 1)],
            ),
            1,
        )];
        let mut calculator = Calculator::new();
        calculator.set_target(Stack::new("Oak Log", 1));
        let actual = calculator.steps().collect::<Vec<_>>();
        assert_eq!(&expected[..], &actual[..]);
    }

    #[test]
    fn calculate_one_step() {
        let expected = [
            (
                &Recipe::new(
                    Stack::new("Oak Log", 1),
                    "Raw Material",
                    vec![Stack::new("Oak Log", 1)],
                ),
                1,
            ),
            (
                &Recipe::new(
                    Stack::new("Charcoal", 1),
                    "Furnace",
                    vec![Stack::new("Oak Log", 1)],
                ),
                1,
            ),
        ];
        let mut calculator = Calculator::new();
        calculator.set_recipe(Recipe::new(
            Stack::new("Charcoal", 1),
            "Furnace",
            vec![Stack::new("Oak Log", 1)],
        ));
        calculator.set_target(Stack::new("Charcoal", 1));
        let actual = calculator.steps().collect::<Vec<_>>();
        assert_eq!(&expected[..], &actual[..]);
    }

    #[test]
    fn calculate_leftovers() {
        let expected = [
            (
                &Recipe::new(
                    Stack::new("Oak Log", 1),
                    "Raw Material",
                    vec![Stack::new("Oak Log", 1)],
                ),
                1,
            ),
            (
                &Recipe::new(
                    Stack::new("Oak Wood Planks", 4),
                    "Crafting Table",
                    vec![Stack::new("Oak Log", 1)],
                ),
                1,
            ),
        ];
        let mut calculator = Calculator::new();
        calculator.set_recipe(Recipe::new(
            Stack::new("Oak Wood Planks", 4),
            "Crafting Table",
            vec![Stack::new("Oak Log", 1)],
        ));
        calculator.set_target(Stack::new("Oak Wood Planks", 1));
        let actual = calculator.steps().collect::<Vec<_>>();
        assert_eq!(&expected[..], &actual[..]);
    }

    #[test]
    fn calculate_leftover_reuse() {
        let expected = [
            (
                &Recipe::new(
                    Stack::new("Oak Log", 1),
                    "Raw Material",
                    vec![Stack::new("Oak Log", 1)],
                ),
                1,
            ),
            (
                &Recipe::new(
                    Stack::new("Oak Wood Planks", 4),
                    "Crafting Table",
                    vec![Stack::new("Oak Log", 1)],
                ),
                1,
            ),
            (
                &Recipe::new(
                    Stack::new("Stick", 4),
                    "Crafting Table",
                    vec![Stack::new("Oak Wood Planks", 2)],
                ),
                1,
            ),
            (
                &Recipe::new(
                    Stack::new("Wooden Shovel", 1),
                    "Crafting Table",
                    vec![Stack::new("Oak Wood Planks", 1), Stack::new("Stick", 2)],
                ),
                1,
            ),
        ];
        let recipes = [
            (
                "Oak Wood Planks".to_string(),
                Recipe::new(
                    Stack::new("Oak Wood Planks", 4),
                    "Crafting Table",
                    vec![Stack::new("Oak Log", 1)],
                ),
            ),
            (
                "Stick".to_string(),
                Recipe::new(
                    Stack::new("Stick", 4),
                    "Crafting Table",
                    vec![Stack::new("Oak Wood Planks", 2)],
                ),
            ),
            (
                "Wooden Shovel".to_string(),
                Recipe::new(
                    Stack::new("Wooden Shovel", 1),
                    "Crafting Table",
                    vec![Stack::new("Oak Wood Planks", 1), Stack::new("Stick", 2)],
                ),
            ),
        ];
        let mut calculator = Calculator::with_recipes(HashMap::from(recipes));
        calculator.set_target(Stack::new("Wooden Shovel", 1));
        let actual = calculator.steps().collect::<Vec<_>>();
        assert_eq!(&expected[..], &actual[..]);
    }

    #[test]
    fn calculate_storage_use() {
        let expected = [
            (
                &Recipe::new(
                    Stack::new("Oak Log", 1),
                    "Raw Material",
                    vec![Stack::new("Oak Log", 1)],
                ),
                1,
            ),
            (
                &Recipe::new(
                    Stack::new("Oak Wood Planks", 4),
                    "Crafting Table",
                    vec![Stack::new("Oak Log", 1)],
                ),
                1,
            ),
            (
                &Recipe::new(
                    Stack::new("Stick", 4),
                    "Crafting Table",
                    vec![Stack::new("Oak Wood Planks", 2)],
                ),
                1,
            ),
            (
                &Recipe::new(
                    Stack::new("Stick", 1),
                    "In Storage",
                    vec![Stack::new("Stick", 1)],
                ),
                1,
            ),
            (
                &Recipe::new(
                    Stack::new("Wooden Shovel", 1),
                    "Crafting Table",
                    vec![Stack::new("Oak Wood Planks", 1), Stack::new("Stick", 2)],
                ),
                1,
            ),
        ];
        let recipes = [
            (
                "Oak Wood Planks".to_string(),
                Recipe::new(
                    Stack::new("Oak Wood Planks", 4),
                    "Crafting Table",
                    vec![Stack::new("Oak Log", 1)],
                ),
            ),
            (
                "Stick".to_string(),
                Recipe::new(
                    Stack::new("Stick", 4),
                    "Crafting Table",
                    vec![Stack::new("Oak Wood Planks", 2)],
                ),
            ),
            (
                "Wooden Shovel".to_string(),
                Recipe::new(
                    Stack::new("Wooden Shovel", 1),
                    "Crafting Table",
                    vec![Stack::new("Oak Wood Planks", 1), Stack::new("Stick", 2)],
                ),
            ),
        ];
        let mut calculator = Calculator::with_recipes(HashMap::from(recipes));
        calculator.set_target(Stack::new("Wooden Shovel", 1));
        calculator.add_resource(Stack::new("Stick", 1));
        let actual = calculator.steps().collect::<Vec<_>>();
        assert_eq!(&expected[..], &actual[..]);
    }
}
