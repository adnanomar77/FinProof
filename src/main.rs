mod ast;
mod bytecode;
mod compiler;
mod lexer;
mod parser;
mod runtime;
mod token;
mod types;
mod verification;
mod vm;

use std::fs;

use clap::{Parser as ClapParser, Subcommand};

use compiler::compile_program;
use lexer::Lexer;
use parser::Parser;
use types::check_program;
use verification::verify_execution_witness;
use vm::execute_bytecode;

#[derive(ClapParser, Debug)]
#[command(
    name = "finproof",
    version = "0.1.0",
    about = "Deterministic financial program verifier",
    disable_help_subcommand = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Check a financial program without executing it.
    Check {
        /// Path to the FinProof source file.
        file: String,
    },

    /// Compile, execute, and verify a financial program.
    Run {
        /// Path to the FinProof source file.
        file: String,
    },

    /// Compile, execute, and verify the execution witness.
    Verify {
        /// Path to the FinProof source file.
        file: String,
    },

    /// Display FinProof version.
    Version,

    /// Display FinProof help.
    Help,
}

fn main() {
    let cli = Cli::parse();

    let (command_name, filename) = match &cli.command {
        Command::Check { file } => ("check", Some(file.as_str())),
        Command::Run { file } => ("run", Some(file.as_str())),
        Command::Verify { file } => ("verify", Some(file.as_str())),
        Command::Version => {
            println!("FinProof v0.1.0");
            return;
        }
        Command::Help => {
            println!("FinProof - deterministic financial program verifier");
            println!();
            println!("Usage:");
            println!("  finproof check <file.fp>");
            println!("  finproof run <file.fp>");
            println!("  finproof verify <file.fp>");
            println!("  finproof version");
            println!("  finproof help");
            return;
        }
    };

    let filename = filename.expect("file is required for this command");

    let source = match fs::read_to_string(filename) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("FP0001 FILE_ERROR: cannot read '{}': {}", filename, error);
            std::process::exit(1);
        }
    };

    println!("=== FinProof ===");
    println!("File: {}", filename);

    // -------------------------------------------------
    // 1. LEX
    // -------------------------------------------------

    let mut lexer = Lexer::new(&source);

    let tokens = match lexer.tokenize() {
        Ok(tokens) => tokens,
        Err(error) => {
            eprintln!("LEX ERROR: {}", error);
            std::process::exit(1);
        }
    };

    println!("Lex: PASS");

    // -------------------------------------------------
    // 2. PARSE
    // -------------------------------------------------

    let mut parser = Parser::new(tokens);

    let program = match parser.parse() {
        Ok(program) => program,
        Err(error) => {
            eprintln!("PARSE ERROR: {}", error);
            std::process::exit(1);
        }
    };

    println!("Parse: PASS");

    // -------------------------------------------------
    // 3. TYPE CHECK
    // -------------------------------------------------

    match check_program(&program) {
        Ok(()) => {
            println!("Type Check: PASS");
        }
        Err(error) => {
            eprintln!("Type Check: FAILED");
            eprintln!("{}", error);
            std::process::exit(1);
        }
    }

    if command_name == "check" {
        println!("FinProof: CHECK SUCCESS");
        return;
    }

    // -------------------------------------------------
    // 4. COMPILE
    // -------------------------------------------------

    let bytecode = match compile_program(&program) {
        Ok(bytecode) => bytecode,
        Err(error) => {
            eprintln!("Compile: FAILED");
            eprintln!("{}", error);
            std::process::exit(1);
        }
    };

    println!("Compile: PASS");

    // -------------------------------------------------
    // 5. EXECUTE
    // -------------------------------------------------

    let result = match execute_bytecode(&bytecode) {
        Ok(result) => result,
        Err(error) => {
            eprintln!("Execution: FAILED");
            eprintln!("{}", error);
            std::process::exit(1);
        }
    };

    println!("Execution: PASS");

    // -------------------------------------------------
    // 6. VERIFY
    // -------------------------------------------------

    match verify_execution_witness(&bytecode, &result.trace) {
        Ok(()) => {
            println!("Verification: PASS");
        }
        Err(error) => {
            eprintln!("Verification: FAILED");
            eprintln!("{}", error);
            std::process::exit(1);
        }
    }

    if command_name == "verify" {
        println!("FinProof: VERIFY SUCCESS");
        return;
    }

    // -------------------------------------------------
    // 7. LEDGER
    // -------------------------------------------------

    println!();
    println!("=== Ledger ===");

    for (index, entry) in result.ledger.iter().enumerate() {
        println!(
            "#{} {} {} {} {}",
            index + 1,
            entry.transaction,
            entry.operation,
            entry.amount,
            entry.currency
        );

        if entry.operation == "debit" {
            println!(
                "   DEBIT  {}: {} -> {}",
                entry.from, entry.from_before, entry.from_after
            );
        } else if entry.operation == "credit" {
            println!(
                "   CREDIT {}: {} -> {}",
                entry.to, entry.to_before, entry.to_after
            );
        } else {
            println!(
                "   FROM {}: {} -> {}",
                entry.from, entry.from_before, entry.from_after
            );
            println!(
                "   TO   {}: {} -> {}",
                entry.to, entry.to_before, entry.to_after
            );
        }
    }

    // -------------------------------------------------
    // 8. FINAL STATE
    // -------------------------------------------------

    println!();
    println!("=== Final State ===");

    let mut accounts: Vec<_> = result.state.accounts.iter().collect();
    accounts.sort_by_key(|(name, _)| *name);

    for (name, account) in accounts {
        println!("{}: {} {}", name, account.balance, account.currency);
    }

    println!();
    println!("FinProof: SUCCESS");
}
