#![feature(trait_alias)]

use std::{env, fs, io, process};

mod codegen;
mod interpreter;
mod lexer;
mod parser;

fn usage(prog: &str) -> ! {
    eprintln!("Usage: {prog} (compile|c|interpret|i) <source.bf>");
    process::exit(1);
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let prog = args.first().unwrap();
    let cmd = args.get(1).unwrap_or_else(|| usage(prog));
    let path = args.get(2).unwrap_or_else(|| usage(prog));

    let contents = fs::read_to_string(path).unwrap_or_else(|err| {
        eprintln!("Failed to read file: {path}: {err}");
        process::exit(1);
    });

    let tokens = lexer::lex(&contents);
    let nodes = parser::parse(&tokens);

    match cmd.as_str() {
        "compile" | "c" => {
            let ir = codegen::generate_ir(&nodes);
            println!("{ir}");
        }
        "interpret" | "i" => {
            let interpreter = interpreter::Interpreter::new(io::stdin(), io::stdout());
            interpreter.run(&nodes).unwrap();
        }
        _ => usage(prog),
    }
}
