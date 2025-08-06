#![feature(trait_alias)]

use std::{env, fs, io, process};

use crate::interpreter::Interpreter;

mod interpreter;
mod lexer;
mod parser;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <path>", args[0]);
        process::exit(1);
    }

    let file_path = &args[1];
    let contents = fs::read_to_string(file_path).unwrap_or_else(|err| {
        eprintln!("Failed to read file: {file_path}: {err}");
        process::exit(1);
    });

    let tokens = lexer::lex(&contents);
    let nodes = parser::parse(&tokens);

    let stdin = io::stdin();
    let stdout = io::stdout();
    let interpreter = Interpreter::new(stdin, stdout);

    interpreter.run(&nodes).unwrap();
}
