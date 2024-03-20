//! The calculator

#![warn(clippy::all)]
#![warn(missing_copy_implementations, missing_docs, rust_2018_idioms)]
#![deny(unsafe_op_in_unsafe_fn, missing_debug_implementations)]
#![cfg_attr(not(debug_assertions), deny(clippy::todo))]

use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Write as IoWrite},
};

use crafting_calculator::{Calculator, Recipe};
use nom::Parser;

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

trait Command {
    fn apply(&self, arguments: &str, state: &mut State);
    fn example(&self) -> &'static str;
    fn short_help(&self) -> &'static str;

    fn long_help(&self) -> &'static str {
        self.short_help()
    }
}

struct Help;

impl Command for Help {
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

impl Command for Load {
    fn apply(&self, arguments: &str, state: &mut State) {
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

impl Command for Print {
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

impl Command for NewRecipe {
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

impl Command for Resource {
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

impl Command for Target {
    fn apply(&self, arguments: &str, state: &mut State) {
        if arguments.is_empty() {
            println!("{}", state.calculator.target());
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

impl Command for Write {
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

const COMMANDS: &[(&str, &dyn Command)] = &[
    ("help", &Help),
    ("load", &Load),
    ("print", &Print),
    ("recipe", &NewRecipe),
    ("resource", &Resource),
    ("target", &Target),
    ("write", &Write),
];

fn cli() -> io::Result<()> {
    let calculator = Calculator::new();
    let mut state = State { calculator };
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

fn main() -> io::Result<()> {
    cli()
}
