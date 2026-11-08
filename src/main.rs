use std::io::{self, Write};

use flint::ast::Stmt;
use flint::compiler::Compiler;
use flint::error::ForgeError;
use flint::eval::Evaluator;
use flint::lexer::Lexer;
use flint::parser::Parser;
use flint::vm::Vm;

fn run_vm(stmts: &[Stmt]) -> Option<Result<String, ForgeError>> {
    match Compiler::compile_program(stmts) {
        Ok(chunk) => {
            let mut vm = Vm::new();
            let g = vm.globals.clone();
            Some(vm.interpret(&chunk, &g).map(|_| String::new()))
        }
        Err(e) if e.to_string().contains("vm-unsupported") => None,
        Err(e) => Some(Err(e)),
    }
}

fn run_source(source: &str, eval: &mut Evaluator) -> Result<String, ForgeError> {
    let tokens = Lexer::new(source).scan().map_err(ForgeError::Scan)?;
    let mut parser = Parser::new(tokens);
    let stmts = parser.parse_program()?;
    if stmts.len() == 1 {
        if let Stmt::Expression(expr) = &stmts[0] {
            let globals = eval.globals.clone();
            let v = eval.eval_expr(expr, &globals)?;
            return Ok(v.render());
        }
    }
    if let Some(res) = run_vm(&stmts) {
        return res;
    }
    eval.run(&stmts)?;
    Ok(String::new())
}

fn run_file(path: &str) -> Result<(), ForgeError> {
    let source = std::fs::read_to_string(path).map_err(|e| {
        ForgeError::Scan(format!("cannot read {}: {}", path, e))
    })?;
    let mut eval = Evaluator::new();
    let out = run_source(&source, &mut eval)?;
    if !out.is_empty() {
        println!("{}", out);
    }
    Ok(())
}

fn repl() -> Result<(), ForgeError> {
    let mut eval = Evaluator::new();
    let stdin = io::stdin();
    loop {
        print!("forge> ");
        io::stdout().flush().ok();
        let mut line = String::new();
        if stdin.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }
        if line == "quit" || line == "exit" {
            break;
        }
        match run_source(&line, &mut eval) {
            Ok(out) => {
                if !out.is_empty() {
                    println!("{}", out);
                }
            }
            Err(e) => println!("{}", e),
        }
    }
    Ok(())
}

const VERSION: &str = "0.1.0";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && (args[1] == "--version" || args[1] == "-V") {
        println!("flint {}", VERSION);
        return;
    }
    if args.len() > 1 && (args[1] == "--help" || args[1] == "-h") {
        println!("flint {}", VERSION);
        println!("usage: flint [file.fl]");
        println!("  with no file, starts the REPL");
        return;
    }
    if args.len() > 1 {
        if let Err(e) = run_file(&args[1]) {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    } else {
        let _ = repl();
    }
}
