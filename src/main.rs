//! The calculator

#![warn(clippy::all)]
#![warn(missing_copy_implementations, missing_docs, rust_2018_idioms)]
#![deny(unsafe_op_in_unsafe_fn, missing_debug_implementations)]
#![cfg_attr(not(debug_assertions), deny(clippy::todo))]

use std::{
    borrow::{Borrow, Cow},
    fs::{self, OpenOptions},
    io::{self, Write as IoWrite},
    rc::Rc,
    sync::RwLock,
};

use clap::Parser;
use nom::Parser as _;
use serde::{Deserialize, Serialize};

use crafting_calculator::{Calculator, Recipe, Stack};

#[allow(missing_docs)]
#[allow(missing_debug_implementations)]
mod gui {
    use std::{cmp::Reverse, fs::OpenOptions, rc, sync::RwLock};

    use crafting_calculator::Stack;
    use rfd::FileDialog;
    use slint::{Model as _, ModelRc, SharedString, StandardListViewItem, VecModel, Weak};

    use crate::{Matcher, ResourceModifier, State};

    slint::include_modules!();

    impl AboutDialog {
        pub(crate) fn real_new() -> Result<Self, slint::PlatformError> {
            let this = Self::new()?;
            let this_weak = this.as_weak();
            this.on_ok_clicked(move || this_weak.unwrap().hide().unwrap());
            Ok(this)
        }
    }

    impl CraftDialog {
        pub(crate) fn real_new(
            main_window: Weak<MainWindow>,
        ) -> Result<Self, slint::PlatformError> {
            let this = Self::new()?;
            let this_weak = this.as_weak();
            this.on_cancel_clicked(move || this_weak.unwrap().hide().unwrap());
            let this_weak = this.as_weak();
            this.on_ok_clicked(move || {
                let this = this_weak.unwrap();
                let name = this.get_item_name();
                let count = this.get_item_count();
                main_window.unwrap().invoke_craft(ItemStack { name, count });
                this.hide().unwrap();
            });
            Ok(this)
        }
    }

    impl ErrorDialog {
        pub(crate) fn real_new() -> Result<Self, slint::PlatformError> {
            let this = Self::new()?;
            let this_weak = this.as_weak();
            this.on_ok_clicked(move || this_weak.unwrap().hide().unwrap());
            Ok(this)
        }

        pub(crate) fn with_message(s: &str) -> Result<Self, slint::PlatformError> {
            let this = Self::real_new()?;
            this.set_message(s.into());
            Ok(this)
        }
    }

    fn show_error(s: &str) {
        ErrorDialog::with_message(s).unwrap().show().unwrap()
    }

    impl MainWindow {
        pub(crate) fn real_new(
            state: rc::Weak<RwLock<State>>,
        ) -> Result<Self, slint::PlatformError> {
            fn reinitialize_ui(this: &MainWindow, state: &State) {
                fn extract_stacks<'recipes>(
                    recipes: impl Iterator<Item = (&'recipes crate::Recipe, usize)>,
                    mut predicate: impl FnMut(&'recipes crate::Recipe) -> bool,
                ) -> (Vec<ItemStack>, Vec<(&'recipes crate::Recipe, usize)>) {
                    let (extracted, remaining) = recipes
                        .into_iter()
                        .partition::<Vec<_>, _>(|(recipe, _)| predicate(recipe));
                    (
                        extracted
                            .into_iter()
                            .map(|(recipe, mult)| recipe.result() * mult)
                            .map(ItemStack::from)
                            .collect(),
                        remaining,
                    )
                }
                let result = state.calculator.target();
                this.set_result(result.into());
                let (raw_materials, recipes) = extract_stacks(state.calculator.steps(), |recipe| {
                    recipe.method() == "Raw Material"
                });
                this.set_raw_materials(mk_vec_model_rc(raw_materials));
                let (in_storage, recipes) = extract_stacks(recipes.into_iter(), |recipe| {
                    recipe.method() == "In Storage"
                });
                this.set_in_storage(mk_vec_model_rc(in_storage));
                let steps = recipes
                    .into_iter()
                    .map(calculator_step_to_recipe)
                    .collect::<Vec<_>>();
                this.set_steps(mk_vec_model_rc(steps));
            }

            let this = Self::new()?;
            this.on_about_clicked(|| AboutDialog::real_new().unwrap().show().unwrap());
            let this_weak = this.as_weak();
            this.on_set_target_clicked(move || {
                TargetDialog::real_new(this_weak.clone())
                    .unwrap()
                    .show()
                    .unwrap();
            });
            let this_weak = this.as_weak();
            let weak_state = state.clone();
            this.on_set_target(move |target| {
                let state = weak_state.upgrade().unwrap();
                state.write().unwrap().calculator.set_target(target.into());
                reinitialize_ui(&this_weak.unwrap(), &state.read().unwrap());
            });
            let this_weak = this.as_weak();
            this.on_craft_clicked(move || {
                CraftDialog::real_new(this_weak.clone())
                    .unwrap()
                    .show()
                    .unwrap()
            });
            let this_weak = this.as_weak();
            let weak_state = state.clone();
            this.on_craft(move |stack| {
                let state = weak_state.upgrade().unwrap();
                // Force the write guard to be released immediately, since it seems to have been
                // held for the entirety of the match expression, resulting in a deadlock when
                // getting the steps.
                let result = state
                    .write()
                    .unwrap()
                    .calculator
                    .perform_craft(&stack.into());
                match result {
                    Ok(()) => reinitialize_ui(&this_weak.unwrap(), &state.read().unwrap()),
                    Err(e) => show_error(&format!("{e}")),
                }
            });

            let this_weak = this.as_weak();
            let weak_state = state.clone();
            this.on_load_recipes_clicked(move || {
                if let Some(filename) = FileDialog::new()
                    .add_filter("Recipe list v1", &["ron", "recipes"])
                    .add_filter("Recipe list v0", &["lst", "recipes"])
                    .set_directory(std::env::current_dir().unwrap_or_else(|_| ".".into()))
                    .pick_file()
                {
                    let Some(filename) = filename.to_str() else {
                        show_error("Cannot open file with non-UTF-8 path");
                        return;
                    };
                    let state = weak_state.upgrade().unwrap();
                    // `res` moved out of if-statement due to deadlock with `reinitialize_ui`.
                    let res = crate::read_recipes(filename, &mut state.write().unwrap());
                    if let Err(e) = res {
                        show_error(&format!("Couldn't read recipes from {filename:?}: {e:?}"));
                    } else {
                        reinitialize_ui(&this_weak.unwrap(), &state.read().unwrap());
                    }
                }
            });
            let weak_state = state.clone();
            this.on_save_recipes_clicked(move || {
                if let Some(filename) = FileDialog::new()
                    .add_filter("Recipe list v1", &["ron", "recipes"])
                    .set_directory(std::env::current_dir().unwrap_or_else(|_| ".".into()))
                    .save_file()
                {
                    let Ok(filename) = filename
                        .to_str()
                        .ok_or_else(|| show_error("Cannot open file with non-UTF-8 path"))
                    else {
                        return;
                    };
                    let Ok(mut out) = OpenOptions::new()
                        .write(true)
                        .truncate(true)
                        .create(true)
                        .read(false)
                        .open(filename)
                        .map_err(|e| show_error(&format!("Cannot open {filename:?}: {e:?}")))
                    else {
                        return;
                    };
                    if let Err(e) = crate::save_recipes(
                        &mut out,
                        &weak_state.upgrade().unwrap().read().unwrap().calculator,
                    ) {
                        let msg = format!("{e:?}");
                        eprintln!("{msg}");
                        show_error(&msg);
                    }
                }
            });
            let this_weak = this.as_weak();
            this.on_add_recipe_clicked(move || {
                RecipeDialog::real_new(this_weak.clone())
                    .unwrap()
                    .show()
                    .unwrap();
            });
            let this_weak = this.as_weak();
            let weak_state = state.clone();
            this.on_add_recipe(move |recipe| {
                let state = weak_state.upgrade().unwrap();
                state
                    .write()
                    .unwrap()
                    .calculator
                    .add_recipes(vec![recipe.into()]);
                reinitialize_ui(&this_weak.unwrap(), &state.read().unwrap());
            });
            let weak_state = state.clone();
            this.on_show_recipes_clicked(move || {
                Recipes::real_new(weak_state.clone())
                    .unwrap()
                    .show()
                    .unwrap()
            });

            let this_weak = this.as_weak();
            this.on_add_resource_clicked(move || {
                let dialog =
                    ResourceDialog::real_new(this_weak.clone(), ResourceModifier::Add).unwrap();
                dialog.set_add_resource(true);
                dialog.show().unwrap();
            });
            let this_weak = this.as_weak();
            let weak_state = state.clone();
            this.on_add_resource(move |stack| {
                let state = weak_state.upgrade().unwrap();
                state.write().unwrap().calculator.add_resource(stack.into());
                reinitialize_ui(&this_weak.unwrap(), &state.read().unwrap());
            });
            let this_weak = this.as_weak();
            this.on_remove_resource_clicked(move || {
                ResourceDialog::real_new(this_weak.clone(), ResourceModifier::Remove)
                    .unwrap()
                    .show()
                    .unwrap();
            });
            let this_weak = this.as_weak();
            let weak_state = state.clone();
            this.on_remove_resource(move |stack| {
                let state = weak_state.upgrade().unwrap();
                state
                    .write()
                    .unwrap()
                    .calculator
                    .remove_resource(stack.into());
                reinitialize_ui(&this_weak.unwrap(), &state.read().unwrap());
            });
            let weak_state = state.clone();
            this.on_show_resources_clicked(move || {
                let mut resources = weak_state
                    .upgrade()
                    .unwrap()
                    .read()
                    .unwrap()
                    .calculator
                    .resources()
                    .collect::<Vec<_>>();
                resources.sort_by_key(|resource| resource.item().to_owned());
                Resources::real_new(resources.into_iter().map(ItemStack::from))
                    .unwrap()
                    .show()
                    .unwrap()
            });

            let this_weak = this.as_weak();
            let weak_state = state.clone();
            this.on_load_resources_clicked(move || {
                if let Some(filename) = FileDialog::new()
                    .add_filter("Resource list", &["resources"])
                    .set_directory(std::env::current_dir().unwrap_or_else(|_| ".".into()))
                    .pick_file()
                {
                    let filename = match filename.to_str() {
                        None => {
                            show_error("Cannot open file with non-UTF-8 path");
                            return;
                        }
                        Some(filename) => filename,
                    };
                    let state = weak_state.upgrade().unwrap();
                    let res = crate::read_resources(filename, &mut state.write().unwrap());
                    if let Err(e) = res {
                        eprintln!("Couldn't read resources from {filename:?}: {e:?}")
                    } else {
                        reinitialize_ui(&this_weak.unwrap(), &state.read().unwrap());
                    };
                }
            });
            let weak_state = state.clone();
            this.on_save_resources_clicked(move || {
                if let Some(filename) = FileDialog::new()
                    .add_filter("Resource list", &["resources"])
                    .set_directory(std::env::current_dir().unwrap_or_else(|_| ".".into()))
                    .save_file()
                {
                    let Ok(filename) = filename
                        .to_str()
                        .ok_or_else(|| show_error("Cannot open file with non-UTF-8 path"))
                    else {
                        return;
                    };
                    let Ok(mut out) = OpenOptions::new()
                        .write(true)
                        .truncate(true)
                        .create(true)
                        .read(false)
                        .open(filename)
                        .map_err(|e| show_error(&format!("Couldn't open {filename:?}: {e:?}")))
                    else {
                        return;
                    };
                    crate::show_resources(
                        &mut out,
                        &mut weak_state.upgrade().unwrap().write().unwrap().calculator,
                    );
                }
            });
            reinitialize_ui(&this, &state.upgrade().unwrap().read().unwrap());
            Ok(this)
        }
    }

    impl RecipeDialog {
        pub(crate) fn real_new(
            main_window: Weak<MainWindow>,
        ) -> Result<Self, slint::PlatformError> {
            let this = Self::new()?;
            let this_weak = this.as_weak();
            this.on_cancel_clicked(move || this_weak.unwrap().hide().unwrap());
            let this_weak = this.as_weak();
            this.on_add_ingredient(move || {
                let this = this_weak.unwrap();
                let ingredients = VecModel::from(
                    this.get_ingredients()
                        .iter()
                        .chain([ItemStack {
                            name: SharedString::from(""),
                            count: 0,
                        }])
                        .collect::<Vec<_>>(),
                );
                this.set_ingredients(ModelRc::new(ingredients));
            });
            let this_weak = this.as_weak();
            this.on_ok_clicked(move || {
                let this = this_weak.unwrap();
                let ingredients = this
                    .get_ingredients()
                    .iter()
                    .filter(|ingredient| !ingredient.name.trim().is_empty() && ingredient.count > 0)
                    .collect::<Vec<_>>();
                if ingredients.is_empty() {
                    show_error("Recipe must include at least one ingredient");
                    return;
                }
                if this.get_method().trim().is_empty() {
                    show_error("Recipe must define a method");
                    return;
                }
                if this.get_result_name().trim().is_empty() || this.get_result_count() <= 0 {
                    show_error("Recipe must have a result");
                    return;
                }
                main_window.unwrap().invoke_add_recipe(Recipe {
                    ingredients: mk_vec_model_rc(ingredients),
                    method: this.get_method(),
                    result: ItemStack {
                        name: this.get_result_name(),
                        count: this.get_result_count(),
                    },
                });
                this_weak.unwrap().hide().unwrap();
            });
            Ok(this)
        }
    }

    impl Recipes {
        pub(crate) fn real_new(
            state: rc::Weak<RwLock<State>>,
        ) -> Result<Self, slint::PlatformError> {
            let this = Self::new()?;
            let recipes = state
                .upgrade()
                .unwrap()
                .read()
                .unwrap()
                .calculator
                .recipes()
                .map(Recipe::from)
                .collect::<Vec<_>>();
            this.set_recipes(mk_vec_model_rc(recipes.clone()));
            let this_weak = this.as_weak();
            this.on_close_clicked(move || this_weak.unwrap().hide().unwrap());
            let this_weak = this.as_weak();
            this.on_search_edited(move |search_text| {
                let this = this_weak.unwrap();
                let matcher = Matcher::from(&*search_text);
                let visible_recipes = recipes
                    .iter()
                    .filter(|recipe| {
                        matcher.matches(&recipe.result.name)
                            || recipe
                                .ingredients
                                .iter()
                                .any(|ingredient| matcher.matches(&ingredient.name))
                    })
                    .cloned()
                    .collect();
                this.set_recipes(mk_vec_model_rc(visible_recipes));
            });
            Ok(this)
        }
    }

    impl ResourceDialog {
        pub(crate) fn real_new(
            main_window: Weak<MainWindow>,
            modifier: ResourceModifier,
        ) -> Result<Self, slint::PlatformError> {
            let this = Self::new()?;
            let this_weak = this.as_weak();
            this.on_cancel_clicked(move || this_weak.unwrap().hide().unwrap());
            let this_weak = this.as_weak();
            this.on_ok_clicked(move || {
                let this = this_weak.unwrap();
                if this.get_item_name().trim().is_empty() {
                    show_error("Resource name must not be empty");
                    return;
                }
                if this.get_item_count() <= 0 {
                    show_error("Resource count must be positive");
                    return;
                }
                let stack = ItemStack {
                    name: this.get_item_name(),
                    count: this.get_item_count(),
                };
                match modifier {
                    ResourceModifier::Add => main_window.unwrap().invoke_add_resource(stack),
                    ResourceModifier::Remove => main_window.unwrap().invoke_remove_resource(stack),
                }
                this_weak.unwrap().hide().unwrap();
            });
            Ok(this)
        }
    }

    impl Resources {
        pub(crate) fn real_new(
            resources: impl IntoIterator<Item = ItemStack>,
        ) -> Result<Self, slint::PlatformError> {
            fn convert_resource(stack: &ItemStack) -> ModelRc<StandardListViewItem> {
                mk_vec_model_rc(vec![
                    stack.name.clone().into(),
                    (&*stack.count.to_string()).into(),
                ])
            }

            let mut resources = resources.into_iter().collect::<Vec<_>>();
            resources.sort();
            let this = Self::new()?;
            let this_weak = this.as_weak();
            this.on_close_clicked(move || this_weak.unwrap().hide().unwrap());
            this.set_resources(mk_vec_model_rc(
                resources.iter().map(convert_resource).collect(),
            ));
            let this_weak = this.as_weak();
            this.on_search_edited(move |search_text| {
                let this = this_weak.unwrap();
                let matcher = Matcher::from(&*search_text);
                let visible_resources = resources
                    .iter()
                    .filter(|&resource| matcher.matches(&resource.name))
                    .map(convert_resource)
                    .collect();
                this.set_resources(mk_vec_model_rc(visible_resources));
            });
            Ok(this)
        }
    }

    impl TargetDialog {
        pub(crate) fn real_new(
            main_window: Weak<MainWindow>,
        ) -> Result<Self, slint::PlatformError> {
            let this = Self::new()?;
            let weak_this = this.as_weak();
            this.on_cancel_clicked(move || weak_this.unwrap().hide().unwrap());
            let weak_this = this.as_weak();
            this.on_ok_clicked(move || {
                let this = weak_this.unwrap();
                if this.get_item_name().trim().is_empty() {
                    show_error("Target name must not be empty");
                    return;
                }
                if this.get_item_count() <= 0 {
                    show_error("Target count must be positive");
                    return;
                }
                this.hide().unwrap();
                main_window.unwrap().invoke_set_target(ItemStack {
                    name: this.get_item_name(),
                    count: this.get_item_count(),
                });
            });
            Ok(this)
        }
    }

    impl From<crate::Recipe> for Recipe {
        fn from(value: crate::Recipe) -> Self {
            Self::from(&value)
        }
    }

    impl From<&'_ crate::Recipe> for Recipe {
        fn from(value: &'_ crate::Recipe) -> Self {
            Self {
                ingredients: mk_vec_model_rc(
                    value.ingredients().iter().map(ItemStack::from).collect(),
                ),
                method: value.method().into(),
                catalysts: mk_vec_model_rc(value.catalysts().map(ItemStack::from).collect()),
                result: value.result().into(),
            }
        }
    }

    impl From<Recipe> for crate::Recipe {
        fn from(value: Recipe) -> Self {
            Self::new(
                value.result.into(),
                value.method,
                value.catalysts.iter().map(Stack::from).collect(),
                value.ingredients.iter().map(Stack::from).collect(),
            )
        }
    }

    impl From<ItemStack> for Stack {
        fn from(value: ItemStack) -> Self {
            Self::new(value.name, value.count as _)
        }
    }

    impl From<&'_ ItemStack> for Stack {
        fn from(value: &'_ ItemStack) -> Self {
            Self::new(&value.name, value.count as _)
        }
    }

    impl From<Stack> for ItemStack {
        fn from(value: Stack) -> Self {
            Self {
                count: value.count() as _,
                name: value.item().into(),
            }
        }
    }

    impl From<&'_ Stack> for ItemStack {
        fn from(value: &'_ Stack) -> Self {
            Self {
                count: value.count() as _,
                name: value.item().into(),
            }
        }
    }

    impl Eq for ItemStack {}

    impl Ord for ItemStack {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            self.name
                .cmp(&other.name)
                .then(Reverse(self.count).cmp(&Reverse(other.count)))
        }
    }

    impl PartialOrd for ItemStack {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }

    pub fn mk_vec_model_rc<T: Clone + 'static>(v: Vec<T>) -> ModelRc<T> {
        ModelRc::new(VecModel::from(v))
    }

    fn calculator_step_to_recipe((r, c): (&crate::Recipe, usize)) -> Recipe {
        let result = r.result();
        let method = r.method();
        let catalysts = r.catalysts();
        let ingredients = r.ingredients();
        Recipe {
            result: ItemStack {
                name: result.item().into(),
                count: (result.count() * c) as _,
            },
            method: method.into(),
            catalysts: mk_vec_model_rc(catalysts.map(ItemStack::from).collect()),
            ingredients: mk_vec_model_rc(
                ingredients
                    .iter()
                    .map(|stack| ItemStack {
                        name: stack.item().into(),
                        count: (stack.count() * c) as _,
                    })
                    .collect(),
            ),
        }
    }
}
use gui::*;

// This module exists to allow easy inspection of the transpiled `ui/MainWindow.slint`, which can
// be found in `./target/<target>/crafting-calculator-<hash>/out/MainWindow.rs`.
// #[allow(missing_docs)]
// #[allow(missing_debug_implementations)]
// mod _gui {
//     include!("../ui/Windows.rs");
// }

fn read_line() -> io::Result<String> {
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(line)
}

fn prompt(prompt: &str) -> io::Result<String> {
    print!("{prompt}: ");
    io::stdout().flush()?;
    let mut s = String::new();
    io::stdin().read_line(&mut s)?;
    if s.is_empty() {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, ""));
    }
    Ok(s.trim().to_string())
}

struct State {
    calculator: Calculator,
}

trait Action {
    /// Perform the action with the given arguments and state.
    fn apply(&self, arguments: &str, state: &mut State);
    /// The user-facing template for the arguments to this action.
    fn example(&self) -> &'static str;
    /// The short help string for this action.
    fn short_help(&self) -> &'static str;

    /// The long help string for this action. The default implementation delegates to
    /// [`short_help`].
    fn long_help(&self) -> &'static str {
        self.short_help()
    }
}

struct Craft;

impl Action for Craft {
    fn apply(&self, arguments: &str, state: &mut State) {
        let target = match arguments.parse() {
            Ok(target) => target,
            Err(e) => {
                eprintln!("Couldn't parse stack: {e}");
                return;
            }
        };
        match state.calculator.perform_craft(&target) {
            Ok(()) => {}
            Err(e) => eprintln!("{e}"),
        }
    }

    fn example(&self) -> &'static str {
        "craft <stack>"
    }

    fn short_help(&self) -> &'static str {
        "Attempt to craft <stack>. Does not perform any crafts if any resources are missing"
    }
}

struct Help;

impl Action for Help {
    fn apply(&self, arguments: &str, _state: &mut State) {
        if arguments.is_empty() {
            let max_width = COMMANDS
                .iter()
                .map(|(_, o)| o.example().len())
                .max()
                .unwrap();
            for (command, msg) in COMMANDS.iter().map(|&(_, o)| (o.example(), o.short_help())) {
                println!("{command:<max_width$}   {msg}");
            }
        } else {
            match COMMANDS
                .iter()
                .find(|&&(c, _)| c == arguments)
                .map(|&(_, o)| o.long_help())
            {
                Some(msg) => println!("{msg}"),
                None => {
                    self.apply("", _state);
                }
            }
        }
    }

    fn example(&self) -> &'static str {
        "help [cmd]"
    }

    fn short_help(&self) -> &'static str {
        "Print this help message or print detailed help about `cmd`."
    }

    fn long_help(&self) -> &'static str {
        "Print information about the available commands. Use `help cmd` to print help about the command `cmd`."
    }
}

#[derive(Deserialize, Serialize)]
enum Recipes {
    V1(Vec<Recipe>),
}

impl Recipes {
    fn unwrap(self) -> Vec<Recipe> {
        match self {
            Self::V1(recipes) => recipes,
        }
    }
}

fn read_recipes(filename: &str, state: &mut State) -> io::Result<()> {
    let s = fs::read_to_string(filename)?;
    let recipes = match ron::from_str::<Recipes>(&s) {
        Ok(recipes) => recipes.unwrap(),
        Err(e) => {
            eprintln!("Couldn't parse recipes as RON: {e:?}");
            eprintln!("Trying v0 representation");
            let (junk, recipes) = Recipe::parse_recipes("Crafting Table")
                .parse(&s)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{e:?}")))?;
            if !junk.is_empty() {
                eprintln!("Found junk data {junk:?} at the end of the recipe file");
            }
            recipes
        }
    };
    state.calculator.add_recipes(recipes);
    Ok(())
}

fn read_resources(filename: &str, state: &mut State) -> io::Result<()> {
    let s = fs::read_to_string(filename)?;
    state.calculator.add_resources(s.lines().flat_map(|line| {
        line.parse()
            .map_err(|e| eprintln!("Couldn't parse {line:?} as resource: {e:?}"))
            .ok()
    }));
    Ok(())
}

struct Load;

impl Action for Load {
    fn apply(&self, arguments: &str, state: &mut State) {
        let Some((method, filename)) = arguments.split_once(' ') else {
            eprintln!("`load` command requires at least two arguments");
            return;
        };
        match method {
            "recipes" => {
                if let Err(e) = read_recipes(filename, state) {
                    eprintln!("Couldn't read recipes from {filename:?}: {e:?}");
                }
            }
            "resources" => {
                if let Err(e) = read_resources(filename, state) {
                    eprintln!("Couldn't read resources from {filename:?}: {e:?}");
                }
            }
            _ => eprintln!("Unknown method {method:?}. Expected `recipes` or `resources`"),
        }
    }

    fn example(&self) -> &'static str {
        "load <recipes|resources> <file>"
    }

    fn short_help(&self) -> &'static str {
        "Read recipes or resources from `file`."
    }
}

fn show_steps(out: &mut dyn IoWrite, calculator: &mut Calculator) {
    for (recipe, count) in calculator.steps() {
        match writeln!(out, "{recipe:.count$}") {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Couldn't write steps: {e:?}");
                return;
            }
        }
    }
}

fn show_resources(out: &mut dyn IoWrite, calculator: &mut Calculator) {
    for stack in calculator.resources() {
        match writeln!(out, "{}", stack) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Couldn't write resources: {e:?}");
                return;
            }
        }
    }
}

fn show_recipes(out: &mut dyn IoWrite, calculator: &Calculator) {
    let mut first_recipe = true;
    for recipe in calculator.recipes() {
        if !first_recipe {
            match writeln!(out) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Coludn't write recipes: {e:?}");
                    return;
                }
            }
        } else {
            first_recipe = false;
        }
        match write!(out, "{recipe}") {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Coludn't write recipes: {e:?}");
                return;
            }
        }
    }
}

fn save_recipes(out: &mut dyn IoWrite, calculator: &Calculator) -> io::Result<()> {
    let recipes = calculator.recipes().cloned().collect();
    let s = ron::ser::to_string(&Recipes::V1(recipes))
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    out.write_all(s.as_bytes())
}

struct Print;

impl Action for Print {
    fn apply(&self, arguments: &str, state: &mut State) {
        match arguments {
            "steps" | "" => show_steps(&mut io::stdout().lock(), &mut state.calculator),
            "resources" => show_resources(&mut io::stdout().lock(), &mut state.calculator),
            "recipes" => show_recipes(&mut io::stdout().lock(), &state.calculator),
            _ => println!("Unknown `what`: {arguments:?}"),
        }
    }

    fn example(&self) -> &'static str {
        "print [what]"
    }

    fn short_help(&self) -> &'static str {
        "Print the current state of the calculator."
    }

    fn long_help(&self) -> &'static str {
        concat!(
            "Print the current state of the calculator.\n",
            "`what` can be `steps`, `resources`, or `recipes`. ",
            "If `what` is omitted, it is assumed to be `steps`.",
        )
    }
}

struct NewRecipe;

impl Action for NewRecipe {
    fn apply(&self, _arguments: &str, state: &mut State) {
        let result = match prompt("Enter result (ex: Oak Planks (4))") {
            Ok(s) => match s.trim().parse() {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Couldn't parse result: {e:?}");
                    return;
                }
            },
            Err(e) => {
                eprintln!("Couldn't get result: {e:?}");
                return;
            }
        };
        let method = match prompt("Enter crafting method") {
            Ok(s) => s.trim().to_string(),
            Err(e) => {
                eprintln!("Couldn't get crafting method: {e:?}");
                return;
            }
        };
        let mut ingredients = vec![];
        loop {
            match prompt("Enter ingredient (leave blank to finish)") {
                Ok(s) if s.trim().is_empty() => break,
                Ok(s) => match s.trim().parse() {
                    Ok(ingredient) => ingredients.push(ingredient),
                    Err(e) => {
                        eprintln!("Couldn't parse ingredient: {e:?}");
                        return;
                    }
                },
                Err(e) => {
                    eprintln!("Couldn't get ingredient: {e:?}");
                    return;
                }
            }
        }
        let recipe = Recipe::new(result, method, vec![], ingredients);
        state.calculator.set_recipe(recipe);
    }

    fn example(&self) -> &'static str {
        "recipe"
    }

    fn short_help(&self) -> &'static str {
        "Add a new recipe to the calculator"
    }

    fn long_help(&self) -> &'static str {
        "Parses the input until the next blank line as a recipe and adds that recipe to the calculator."
    }
}

struct Resource;

impl Action for Resource {
    fn apply(&self, arguments: &str, state: &mut State) {
        macro_rules! parse_resource {
            ($s:ident) => {
                match $s.parse() {
                    Ok(resource) => resource,
                    Err(e) => {
                        eprintln!("Couldn't parse resource: {e:?}");
                        return;
                    }
                }
            };
        }
        let (method, resource) = match arguments.split_once(char::is_whitespace) {
            Some(("add", stack)) => {
                let stack = stack.trim();
                (ResourceModifier::Add, parse_resource!(stack))
            }
            Some(("remove", stack)) => {
                let stack = stack.trim();
                (ResourceModifier::Remove, parse_resource!(stack))
            }
            Some((method, _)) => {
                eprintln!("Invalid method {method:?}");
                return;
            }
            None => {
                let method = match prompt("add/remove resource?").as_ref().map(|s| &**s) {
                    Ok("add") => ResourceModifier::Add,
                    Ok("remove") => ResourceModifier::Remove,
                    Ok(method) => {
                        eprintln!("Invalid method {method:?}");
                        return;
                    }
                    Err(e) => {
                        eprintln!("Couldn't get method: {e:?}");
                        return;
                    }
                };
                let stack = match prompt("Enter resource") {
                    Ok(s) => parse_resource!(s),
                    Err(e) => {
                        eprintln!("Couldn't get resource: {e:?}");
                        return;
                    }
                };
                (method, stack)
            }
        };
        match method {
            ResourceModifier::Add => state.calculator.add_resource(resource),
            ResourceModifier::Remove => state.calculator.remove_resource(resource),
        }
    }

    fn example(&self) -> &'static str {
        "resource [<add|remove> stack]"
    }

    fn short_help(&self) -> &'static str {
        "Adds or removes `stack` as a resource that is already available for crafting"
    }

    fn long_help(&self) -> &'static str {
        "Adds or removes `stack` as a resource that is already available and therefore does not need to be crafted"
    }
}

enum ResourceModifier {
    Add,
    Remove,
}

struct Target;

impl Action for Target {
    fn apply(&self, arguments: &str, state: &mut State) {
        if arguments.is_empty() {
            println!("Current target is {}", state.calculator.target());
            return;
        }
        let target = match arguments.parse() {
            Ok(target) => target,
            Err(e) => {
                eprintln!("{e}");
                return;
            }
        };
        state.calculator.set_target(target);
    }

    fn example(&self) -> &'static str {
        "target [stack]"
    }

    fn short_help(&self) -> &'static str {
        "Sets the calculator to target `stack` or prints the current target"
    }

    fn long_help(&self) -> &'static str {
        "If `stack` is given, the calculator's target is set to `stack`. Otherwise, prints the calculator's current target."
    }
}

struct Write;

impl Action for Write {
    fn apply(&self, arguments: &str, state: &mut State) {
        let open_file = |f| {
            OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .read(false)
                .open(f)
        };
        let (f, what) = if let Some(what) = arguments.split_whitespace().last() {
            if what == arguments.trim() {
                (open_file(what), "recipes")
            } else {
                let file = arguments.strip_suffix(what).unwrap().trim();
                (open_file(file), what)
            }
        } else {
            eprintln!("Can't write state with no `file` argument.");
            return;
        };
        let mut f = match f {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Couldn't open file for writing: {e:?}");
                return;
            }
        };
        match what {
            "steps" => show_steps(&mut f, &mut state.calculator),
            "resources" => show_resources(&mut f, &mut state.calculator),
            "recipes" => {
                if let Err(e) = save_recipes(&mut f, &state.calculator) {
                    eprintln!("{e:?}");
                }
            }
            _ => eprintln!("Can't write unknown item type {what:?}"),
        }
    }

    fn example(&self) -> &'static str {
        "write <file> [what]"
    }

    fn short_help(&self) -> &'static str {
        "Similar to `print what` but writes to `file` and defaults to `recipes`."
    }

    fn long_help(&self) -> &'static str {
        concat!(
            "Write the current state of the calculator to `file`.\n",
            "`what` can be `steps`, `resources`, or `recipes`. ",
            "If `what` is omitted, it is assumed to be `recipes`.",
        )
    }
}

const COMMANDS: &[(&str, &dyn Action)] = &[
    ("craft", &Craft),
    ("help", &Help),
    ("load", &Load),
    ("print", &Print),
    ("recipe", &NewRecipe),
    ("resource", &Resource),
    ("target", &Target),
    ("write", &Write),
];

/// A string predicate.
enum Matcher<'query> {
    /// Some part of the string exactly matches the query.
    CaseSensitive(&'query str),
    /// Some part of the string matches the query ignoring case.
    CaseInsensitive(Cow<'query, str>),
}

impl Matcher<'_> {
    fn matches(&self, s: &str) -> bool {
        match self {
            Self::CaseSensitive(query) => s.contains(query),
            Self::CaseInsensitive(query) => s.to_lowercase().contains::<&str>(query.borrow()),
        }
    }
}

impl<'query> From<&'query str> for Matcher<'query> {
    fn from(query: &'query str) -> Self {
        if query.chars().any(|c| c.is_uppercase()) {
            Self::CaseSensitive(query)
        } else {
            Self::CaseInsensitive(Cow::Borrowed(query))
        }
    }
}

fn cli(mut state: State) -> io::Result<()> {
    loop {
        print!("$ ");
        io::stdout().flush()?;
        let line = read_line()?;
        if line.is_empty() {
            println!();
            break Ok(());
        }
        let mut words = line.split_whitespace();
        let command = match words.next() {
            Some(word) => word,
            None => continue,
        };
        let arguments = line.strip_prefix(command).unwrap().trim();
        match COMMANDS
            .iter()
            .find(|(c, _)| c.strip_prefix(command).is_some())
        {
            Some((_, f)) => f.apply(arguments, &mut state),
            None => Help.apply("", &mut state),
        }
    }
}

#[derive(Parser, Debug)]
struct Args {
    /// A file of recipes that should be loaded into the calculator during start-up. May be
    /// specified any number of times. If specified more than once, all specified files will be
    /// loaded.
    #[arg(short, long)]
    recipes: Vec<String>,
    /// Start the calculator in GUI mode.
    #[arg(short = 'g', long)]
    use_gui: bool,
    /// A file of resources in storage that should be loaded into the calculator during start-up.
    /// May be specified any number of times. If specified more than once, all specified files will
    /// be loaded.
    #[arg(long)]
    resources: Vec<String>,
    /// The initial target for the calculator. Should be given in the form "Item Name (count)".
    /// Default value is "Air (1)".
    #[arg(short, long)]
    target: Option<Stack>,
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    let use_gui = args.use_gui;
    let mut state = State {
        calculator: Calculator::new(),
    };
    for file in args.recipes {
        read_recipes(&file, &mut state)?;
    }
    for file in args.resources {
        read_resources(&file, &mut state)?;
    }
    if let Some(target) = args.target {
        state.calculator.set_target(target);
    }
    if use_gui {
        let state = Rc::new(RwLock::new(state));
        MainWindow::real_new(Rc::downgrade(&state))
            .unwrap()
            .run()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
    } else {
        cli(state)?;
    }
    Ok(())
}
