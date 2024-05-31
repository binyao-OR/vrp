//! A command line interface to *Vehicle Routing Problem* solver.
//!

use std::fs::File;
use std::io::BufReader;
use std::io::prelude::*;
use std::sync::Arc;
use vrp_cli::extensions::solve::config::read_config;
use vrp_cli::get_solution_serialized;
use vrp_pragmatic::format::problem::PragmaticProblem;

#[cfg(test)]
#[path = "../tests/unit/main_test.rs"]
mod main_test;

#[cfg(not(target_arch = "wasm32"))]
mod commands;

fn main() {
    let mut problem_file = File::open("/Users/yaobin/Downloads/problem42.csv").unwrap();
    let mut config_file = File::open("/Users/yaobin/Downloads/config.csv").unwrap();
    let mut matrix_file = File::open("/Users/yaobin/Downloads/matrix.csv").unwrap();

    let mut problem = String::new();
    let mut config = String::new();
    let mut matrix1 = String::new();

    problem_file.read_to_string(&mut problem).unwrap();
    config_file.read_to_string(&mut config).unwrap();
    matrix_file.read_to_string(&mut matrix1).unwrap();

    let mut matrices = vec![];
    matrices.push(matrix1);

    let result =
        if matrices.is_empty() { problem.read_pragmatic() } else { (problem, matrices).read_pragmatic() }
            .map_err(|errs| errs.into())
            .and_then(|problem| {
                read_config(BufReader::new(config.as_bytes()))
                    .map(|config| (problem, config))
            })
            .and_then(|(problem, config)| get_solution_serialized(Arc::new(problem), config)).unwrap();

    println!("result is: {}", result);
}

#[cfg(not(target_arch = "wasm32"))]
mod cli {
    use super::commands::import::{get_import_app, run_import};
    use super::commands::solve::{get_solve_app, run_solve};
    use crate::commands::analyze::{get_analyze_app, run_analyze};
    use crate::commands::check::{get_check_app, run_check};
    use crate::commands::create_write_buffer;
    use crate::commands::generate::{get_generate_app, run_generate};
    use clap::{ArgMatches, Command};
    use std::process;

    pub fn run_app() {
        run_subcommand(get_app().get_matches());
    }

    pub fn get_app() -> Command {
        Command::new("Vehicle Routing Problem Solver")
            .version("1.23.0")
            .author("Ilya Builuk <ilya.builuk@gmail.com>")
            .about("A command line interface to Vehicle Routing Problem solver")
            .subcommand(get_analyze_app())
            .subcommand(get_solve_app())
            .subcommand(get_import_app())
            .subcommand(get_check_app())
            .subcommand(get_generate_app())
    }

    pub fn run_subcommand(arg_matches: ArgMatches) {
        if let Err(err) = match arg_matches.subcommand() {
            Some(("analyze", analyze_matches)) => run_analyze(analyze_matches, create_write_buffer),
            Some(("solve", solve_matches)) => run_solve(solve_matches, create_write_buffer),
            Some(("import", import_matches)) => run_import(import_matches),
            Some(("check", check_matches)) => run_check(check_matches),
            Some(("generate", generate_matches)) => run_generate(generate_matches),
            _ => {
                eprintln!("no subcommand was used. Use -h to print help information.");
                process::exit(1);
            }
        } {
            eprintln!("{err}");
            process::exit(1)
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod cli {
    pub fn run_app() {}
}
