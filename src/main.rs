//! The calculator

#![warn(clippy::all)]
#![warn(missing_copy_implementations, missing_docs, rust_2018_idioms)]
#![deny(unsafe_op_in_unsafe_fn, missing_debug_implementations)]
#![cfg_attr(not(debug_assertions), deny(clippy::todo))]

use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Write as IoWrite},
    rc::Rc,
    sync::RwLock,
};

use clap::Parser;
use crafting_calculator::{Calculator, Recipe};

#[cfg(feature = "gui")]
#[allow(missing_docs)]
#[allow(missing_debug_implementations)]
mod gui {
    use std::{rc, sync::RwLock};

    use crafting_calculator::Stack;
    use slint::{Model as _, ModelRc, SharedString, VecModel, Weak};

    use crate::State;

    slint::include_modules!();

    impl MainWindow {
        pub(crate) fn real_new(
            state: rc::Weak<RwLock<State>>,
        ) -> Result<Self, slint::PlatformError> {
            let this = Self::new()?;
            let this_weak = this.as_weak();
            let weak_state = state.clone();
            this.on_set_target_clicked(move || {
                TargetDialog::real_new(this_weak.clone(), weak_state.clone())
                    .unwrap()
                    .show()
                    .unwrap();
            });
            let this_weak = this.as_weak();
            let weak_state = state.clone();
            this.on_set_target(move || {
                let state = weak_state.upgrade().unwrap();
                let state = state.read().unwrap();
                let result = state.calculator.target();
                let this = this_weak.unwrap();
                this.set_result(result.into());
                let steps = state
                    .calculator
                    .steps()
                    .map(calculator_step_to_recipe)
                    .collect::<Vec<_>>();
                this.set_steps(mk_vec_model_rc(steps));
            });
            let this_weak = this.as_weak();
            let weak_state = state.clone();
            this.on_add_recipe_clicked(move || {
                let popup = RecipeDialog::real_new(this_weak.clone(), weak_state.clone()).unwrap();
                popup.show().unwrap();
            });
            this.invoke_set_target();
            Ok(this)
        }
    }

    impl RecipeDialog {
        pub(crate) fn real_new(
            main_window: Weak<MainWindow>,
            state: rc::Weak<RwLock<State>>,
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
                let result = Stack::new(this.get_result_name(), this.get_result_count() as _);
                let method = this.get_method();
                let ingredients = this
                    .get_ingredients()
                    .iter()
                    .map(Stack::from)
                    .collect::<Vec<_>>();
                state
                    .upgrade()
                    .unwrap()
                    .write()
                    .unwrap()
                    .calculator
                    .add_recipes(vec![crate::Recipe::new(result, method, ingredients)]);
                this_weak.unwrap().hide().unwrap();
                main_window.unwrap().invoke_set_target();
            });
            Ok(this)
        }
    }

    impl TargetDialog {
        pub(crate) fn real_new(
            main_window: Weak<MainWindow>,
            state: rc::Weak<RwLock<State>>,
        ) -> Result<Self, slint::PlatformError> {
            let this = Self::new()?;
            let weak_this = this.as_weak();
            this.on_cancel_clicked(move || weak_this.unwrap().hide().unwrap());
            let weak_this = this.as_weak();
            this.on_ok_clicked(move || {
                let this = weak_this.unwrap();
                if this.get_item_name().trim().is_empty() {
                    return;
                }
                state
                    .upgrade()
                    .unwrap()
                    .write()
                    .unwrap()
                    .calculator
                    .set_target(Stack::new(this.get_item_name(), this.get_item_count() as _));
                this.hide().unwrap();
                main_window.unwrap().invoke_set_target();
            });
            Ok(this)
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

    pub fn mk_vec_model_rc<T: Clone + 'static>(v: Vec<T>) -> ModelRc<T> {
        ModelRc::new(VecModel::from(v))
    }

    fn calculator_step_to_recipe((r, c): (&crate::Recipe, usize)) -> Recipe {
        let result = r.result();
        let method = r.method();
        let ingredients = r.ingredients();
        Recipe {
            result: ItemStack {
                name: result.item().into(),
                count: (result.count() * c) as _,
            },
            method: method.into(),
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
#[cfg(feature = "gui")]
use gui::*;

// This module exists to allow easy inspection of the transpiled `ui/MainWindow.slint`, which can
// be found in `./target/<target>/crafting-calculator-<hash>/out/MainWindow.rs`.
// #[cfg(feature = "gui")]
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
    fn apply(&self, arguments: &str, state: &mut State);
    fn example(&self) -> &'static str;
    fn short_help(&self) -> &'static str;

    fn long_help(&self) -> &'static str {
        self.short_help()
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

struct Load;

impl Action for Load {
    fn apply(&self, arguments: &str, state: &mut State) {
        use nom::Parser;

        let calculator = &mut state.calculator;
        let filename = arguments;
        let mut f = match File::open(filename) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Couldn't open file {filename:?}: {e:?}");
                return;
            }
        };
        let recipes = {
            let mut s = String::new();
            match f.read_to_string(&mut s) {
                Ok(_) => {}
                Err(e) => eprintln!("Couldn't read recipe file {filename:?}: {e:?}"),
            }
            match Recipe::parse_recipes("Crafting Table").parse(&s) {
                Ok(("", recipes)) => recipes,
                Ok((junk, recipes)) => {
                    eprintln!("Found junk data {junk:?} at the end of the recipe file");
                    recipes
                }
                Err(e) => {
                    let e = io::Error::new(io::ErrorKind::InvalidData, format!("{e:?}"));
                    eprintln!("Couldn't parse recipe file {filename:?}: {e:?}");
                    return;
                }
            }
        };
        calculator.add_recipes(recipes);
    }

    fn example(&self) -> &'static str {
        "load <file>"
    }

    fn short_help(&self) -> &'static str {
        "Read recipes from `file`."
    }
}

fn write_steps(out: &mut dyn IoWrite, calculator: &mut Calculator) {
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

fn write_resources(out: &mut dyn IoWrite, calculator: &mut Calculator) {
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

fn write_recipes(out: &mut dyn IoWrite, calculator: &mut Calculator) {
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

struct Print;

impl Action for Print {
    fn apply(&self, arguments: &str, state: &mut State) {
        match arguments {
            "steps" | "" => write_steps(&mut io::stdout().lock(), &mut state.calculator),
            "resources" => write_resources(&mut io::stdout().lock(), &mut state.calculator),
            "recipes" => write_recipes(&mut io::stdout().lock(), &mut state.calculator),
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
            Ok(s) => match s.parse() {
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
            Ok(s) => s,
            Err(e) => {
                eprintln!("Couldn't get crafting method: {e:?}");
                return;
            }
        };
        let mut ingredients = vec![];
        loop {
            match prompt("Enter ingredient (leave blank to finish)") {
                Ok(s) if s.is_empty() => break,
                Ok(s) => match s.parse() {
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
        let recipe = Recipe::new(result, method, ingredients);
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
                let file = arguments.strip_suffix(what).unwrap();
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
            "steps" => write_steps(&mut f, &mut state.calculator),
            "resources" => write_resources(&mut f, &mut state.calculator),
            "recipes" => write_recipes(&mut f, &mut state.calculator),
            _ => {
                let mut f = match open_file(arguments.trim()) {
                    Ok(f) => f,
                    Err(e) => {
                        eprintln!("Couldn't open file: {e:?}");
                        return;
                    }
                };
                write_recipes(&mut f, &mut state.calculator);
            }
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
    ("help", &Help),
    ("load", &Load),
    ("print", &Print),
    ("recipe", &NewRecipe),
    ("resource", &Resource),
    ("target", &Target),
    ("write", &Write),
];

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
    #[arg(short, long)]
    recipes: Vec<String>,
    #[cfg(feature = "gui")]
    #[arg(short = 'g', long)]
    use_gui: bool,
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    #[cfg(feature = "gui")]
    let use_gui = args.use_gui;
    #[cfg(not(feature = "gui"))]
    let use_gui = false;
    let mut state = State {
        calculator: Calculator::new(),
    };
    for file in args.recipes {
        Load.apply(&file, &mut state);
    }
    if use_gui {
        #[cfg(feature = "gui")]
        {
            let state = Rc::new(RwLock::new(state));
            MainWindow::real_new(Rc::downgrade(&state))
                .unwrap()
                .run()
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        }
    } else {
        cli(state)?;
    }
    Ok(())
}
