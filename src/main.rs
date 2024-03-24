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
use crafting_calculator::{Calculator, Recipe, Stack};

#[cfg(feature = "gui")]
#[allow(missing_docs)]
#[allow(missing_debug_implementations)]
mod gui {
    use crafting_calculator::Stack;
    use slint::{Model, ModelRc, SharedString, VecModel};

    slint::slint! {
        import { HorizontalBox, LineEdit, SpinBox, StandardButton } from "std-widgets.slint";

        export component TargetDialog inherits Dialog {
            out property <string> item_name <=> name.text;
            out property <int> item_count <=> count.value;
            callback cancel_clicked();
            callback ok_clicked();
            forward-focus: name;
            FocusScope {
                HorizontalBox {
                    name := LineEdit {
                        enabled: true;
                        accepted => { root.ok_clicked(); }
                    }
                    count := SpinBox {
                        enabled: true;
                        minimum: 1;
                        maximum: 2147483647;
                        horizontal-stretch: 0;
                    }
                }
                key-pressed(event) => {
                    if (event.text == Key.Escape) {
                        root.cancel_clicked();
                        accept
                    } else if (event.text == Key.Return) {
                        root.ok_clicked();
                        accept
                    } else {
                        reject
                    }
                }
            }

            StandardButton { kind: cancel; }
            StandardButton { kind: ok; }
        }
    }

    slint::slint! {
        import {
            Button,
            HorizontalBox,
            LineEdit,
            SpinBox,
            StandardButton
        } from "std-widgets.slint";

        struct RItemCount {
            name: string,
            count: int,
        }

        export component RecipeDialog inherits Dialog {
            out property <string> result_name <=> res_name.text;
            out property <int> result_count <=> res_count.value;
            out property <string> method <=> m.text;
            in-out property <[RItemCount]> ingredients: [{ name: "", count: 0 }];
            callback add_ingredient();
            callback cancel_clicked();
            callback ok_clicked();
            forward-focus: res_name;
            FocusScope {
                VerticalLayout {
                    HorizontalBox {
                        res_name := LineEdit {
                            enabled: true;
                            accepted => { root.ok_clicked(); }
                        }
                        res_count := SpinBox {
                            enabled: true;
                            minimum: 1;
                            maximum: 2147483647;
                            horizontal-stretch: 0;
                        }
                    }
                    m := LineEdit {
                        enabled: true;
                        accepted => { root.ok_clicked(); }
                    }
                    for ingredient[i] in ingredients : FocusScope {
                        HorizontalBox {
                            name := LineEdit {
                                text: ingredient.name;
                                enabled: true;
                                edited(s) => { root.ingredients[i].name = s; }
                                accepted => {
                                    self.edited(self.text);
                                    root.ok_clicked();
                                }
                            }
                            count := SpinBox {
                                value: ingredient.count;
                                enabled: true;
                                minimum: 1;
                                maximum: 2147483647;
                                edited(n) => { root.ingredients[i].count = n; }
                                horizontal-stretch: 0;
                            }
                        }
                        focus-changed-event => {
                            name.edited(name.text);
                            count.edited(count.value);
                        }
                    }
                    Button {
                        text: "+";
                    }
                    Text { vertical-stretch: 1; }
                }
                key-pressed(event) => {
                    if (event.text == Key.Escape) {
                        root.cancel_clicked();
                        accept
                    } else if (event.text == Key.Return) {
                        root.ok_clicked();
                        accept
                    } else {
                        reject
                    }
                }
            }

            StandardButton { kind: cancel; }
            StandardButton { kind: ok; }
        }
    }

    slint::slint! {
        import { StandardButton } from "std-widgets.slint";

        export component ErrorDialog inherits Dialog {
            in property <string> message <=> msg.text;
            callback ok_clicked();

            msg := Text {}

            StandardButton { kind: ok; }
        }
    }

    impl TargetDialog {
        pub fn real_new() -> Result<Self, slint::PlatformError> {
            let this = Self::new()?;
            let weak = this.as_weak();
            this.on_cancel_clicked(move || weak.unwrap().window().hide().unwrap());
            Ok(this)
        }
    }
    impl RecipeDialog {
        pub fn real_new() -> Result<Self, slint::PlatformError> {
            let this = Self::new()?;
            let weak = this.as_weak();
            this.on_cancel_clicked(move || weak.unwrap().window().hide().unwrap());
            let weak = this.as_weak();
            this.on_add_ingredient(move || {
                let this = weak.unwrap();
                let ingredients = VecModel::from(
                    this.get_ingredients()
                        .iter()
                        .chain([RItemCount {
                            name: SharedString::from(""),
                            count: 0,
                        }])
                        .collect::<Vec<_>>(),
                );
                this.set_ingredients(ModelRc::new(ingredients));
            });
            Ok(this)
        }
    }
    impl ErrorDialog {
        pub fn real_new() -> Result<Self, slint::PlatformError> {
            let this = Self::new()?;
            let weak = this.as_weak();
            this.on_ok_clicked(move || weak.unwrap().window().hide().unwrap());
            Ok(this)
        }
    }
    slint::include_modules!();
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
}
#[cfg(feature = "gui")]
use gui::*;

// This module exists to allow easy inspection of the transpiled `ui/MainWindow.slint`, which can
// be found in `./target/<target>/crafting-calculator-<hash>/out/MainWindow.rs`.
// #[cfg(feature = "gui")]
// #[allow(missing_docs)]
// #[allow(missing_debug_implementations)]
// mod _gui {
//     include!("../ui/MainWindow.rs");
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
        let resource = if arguments.is_empty() {
            match prompt("Enter resource") {
                Ok(s) => parse_resource!(s),
                Err(e) => {
                    eprintln!("Couldn't get resource: {e:?}");
                    return;
                }
            }
        } else {
            parse_resource!(arguments)
        };
        state.calculator.add_resource(resource);
    }

    fn example(&self) -> &'static str {
        "resource [stack]"
    }

    fn short_help(&self) -> &'static str {
        "Adds `stack` as a resource that is already available for crafting"
    }

    fn long_help(&self) -> &'static str {
        "Adds `stack` as a resource that is already available and therefore does not need to be crafted"
    }
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
            let main_window = MainWindow::new().unwrap();
            let weak_main_window = main_window.as_weak();
            let state = Rc::new(RwLock::new(state));
            let weak_state = Rc::downgrade(&state);
            main_window.on_set_target_clicked(move || {
                let popup = TargetDialog::real_new().unwrap();
                let weak_popup = popup.as_weak();
                let weak_main_window = weak_main_window.clone();
                let weak_state = weak_state.clone();
                popup.on_ok_clicked(move || {
                    let popup = weak_popup.unwrap();
                    weak_state
                        .upgrade()
                        .unwrap()
                        .write()
                        .unwrap()
                        .calculator
                        .set_target(Stack::new(
                            popup.get_item_name(),
                            popup.get_item_count() as _,
                        ));
                    popup.hide().unwrap();
                    weak_main_window.upgrade().unwrap().invoke_set_target();
                });
                popup.show().unwrap();
            });
            let weak_main_window = main_window.as_weak();
            let weak_state = Rc::downgrade(&state);
            main_window.on_set_target(move || {
                let state = weak_state.upgrade().unwrap();
                let state = state.read().unwrap();
                let result = state.calculator.target();
                let main_window = weak_main_window.upgrade().unwrap();
                main_window.set_result(result.into());
                let steps = state
                    .calculator
                    .steps()
                    .map(|(r, c)| {
                        let result = r.result();
                        let method = r.method();
                        let ingredients = r.ingredients();
                        gui::Recipe {
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
                    })
                    .collect::<Vec<_>>();
                main_window.set_steps(mk_vec_model_rc(steps));
            });
            let weak_main_window = main_window.as_weak();
            let weak_state = Rc::clone(&state);
            main_window.on_add_recipe_clicked(move || {
                let popup = RecipeDialog::real_new().unwrap();
                let weak_popup = popup.as_weak();
                let weak_main_window = weak_main_window.clone();
                let weak_state = Rc::clone(&weak_state);
                popup.on_ok_clicked(move || {
                    use slint::Model;

                    let popup = weak_popup.upgrade().unwrap();
                    let result = Stack::new(popup.get_result_name(), popup.get_result_count() as _);
                    let method = popup.get_method();
                    let ingredients = popup
                        .get_ingredients()
                        .iter()
                        .map(|s| Stack::new(s.name, s.count as _))
                        .collect::<Vec<_>>();
                    weak_state
                        .write()
                        .unwrap()
                        .calculator
                        .add_recipes(vec![Recipe::new(result, method, ingredients)]);
                    weak_popup.upgrade().unwrap().hide().unwrap();
                    weak_main_window.upgrade().unwrap().invoke_set_target();
                });
                popup.show().unwrap();
            });
            main_window
                .run()
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        }
    } else {
        cli(state)?;
    }
    Ok(())
}
