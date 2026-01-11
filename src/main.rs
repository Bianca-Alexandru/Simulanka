
// files tldr:
// - types.rs    : data structures
// - lexer.rs    : converts text to tokens
// - parser.rs   : converts tokens to syntax tree
// - semantic.rs : checks for errors before running
// - eval.rs     : runs the code
// - world.rs    : simulation logic
// - evolution.rs: evolutionary alg logic
// - gui.rs      : visual display

mod types;
mod lexer;
mod parser;
mod eval;
mod world;
mod semantic;
mod evolution;
mod gui;

use std::sync::Arc;
use eframe::egui;
use types::*;
use lexer::lexer;
use parser::Parser;
use semantic::validate_program;
use gui::SimApp;

fn main() {
    // get command line arguments
    let args: Vec<String> = std::env::args().collect();
    
    // check usage
    if args.len() < 2 {
        println!("Usage: simulanka <file.txt>");
        return;
    }

    // read and parse the source file
    let input = match std::fs::read_to_string(&args[1]) {
        Ok(text) => text,
        Err(e) => { println!("Error reading file: {}", e); return; }
    };

    let tokens = lexer(&input);
    let mut parser = Parser::new(tokens);
    
    let program = match parser.parse_program() {
        Ok(p) => Arc::new(p),
        Err(e) => { println!("Parse Error: {}", e); return; }
    };

    // check for semantic errors
    if let Err(errors) = validate_program(&program) {
        println!("Semantic Errors found:");
        for e in errors { println!("  - {}", e); }
        return;
    }

    // create world instances
    let generations = program.evolve_block.generations;
    let num_instances = program.evolve_block.instances;
    
    let mut instances = Vec::new();
    for i in 0..num_instances {
        let mut w = World::new(program.clone(), i);
        w.spawn();
        instances.push(w);
    }

    run_with_gui(instances, program, generations, num_instances);
}

// run gui
fn run_with_gui(instances: Vec<World>, program: Arc<Program>, generations: i32, num_instances: i32) {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 750.0])
            .with_min_inner_size([700.0, 700.0]),
        ..Default::default()
    };
    let _ = eframe::run_native(
        "Simulanka Evolution Simulator",
        options,
        Box::new(move |_| {
            Ok(Box::new(SimApp::new(instances, program, generations, num_instances)))
        }),
    );
}
