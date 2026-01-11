// semantic.rs -> catches semantic errors

use std::collections::HashMap;
use crate::types::*;

#[derive(Debug, Clone, PartialEq)]
enum Type {
    Int,
    String,
    Bool,
    List,
    Object,
    Environment,
    Unknown,
}

pub fn validate_program(prog: &Program) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    
    // 1. Map out the environment
    let mut globals = HashMap::new();
    globals.insert("width".to_string(), Type::Int);
    globals.insert("height".to_string(), Type::Int);
    globals.insert("steps".to_string(), Type::Int);
    globals.insert("environment".to_string(), Type::Environment);
    
    //known species properties
    let mut known_props = HashMap::new();
    known_props.insert("x".to_string(), Type::Int);
    known_props.insert("y".to_string(), Type::Int);
    known_props.insert("species".to_string(), Type::String);
    known_props.insert("fitness".to_string(), Type::Int);

    for species in prog.species_block.values() {
        for (prop, exp) in &species.properties {
            known_props.insert(prop.clone(), infer_type(exp));
        }
    }

    //2. Validate Routines
    for (name, routine) in &prog.routines_block {
        let mut locals = globals.clone();
        locals.insert("self".to_string(), Type::Object);
        check_commands(&routine.body, &locals, &known_props, &mut errors, name);
    }

    //3. Validate Blocks
    check_commands(&prog.spawns_block, &globals, &known_props, &mut errors, "SPAWN");
    
    //validate Fitness Block
    {
        let mut locals = globals.clone();
        locals.insert("self".to_string(), Type::Object);
        check_commands(&prog.fitness_block.commands, &locals, &known_props, &mut errors, "FITNESS");
    }

    for rule in &prog.mutations_block {
        if let Some(body) = &rule.body {
            let mut locals = globals.clone();
            if rule.action == "crossover" {
                locals.insert("parent1".to_string(), Type::Object);
                locals.insert("parent2".to_string(), Type::Object);
                locals.insert("child".to_string(), Type::Object);
            } else {
                locals.insert("self".to_string(), Type::Object);
            }
            check_commands(body, &locals, &known_props, &mut errors, &rule.action);
        }
    }

    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

fn check_commands(
    cmds: &[Command],
    env: &HashMap<String, Type>,
    props: &HashMap<String, Type>,
    errors: &mut Vec<String>,
    context: &str,
) {
    let mut current_env = env.clone();
    for cmd in cmds {
        match cmd {
            Command::Assign { target, value, line} => {
                let val_type = check_exp(value, &current_env, props, errors, context);
                match target {
                    Exp::Var(name, _) => { current_env.insert(name.clone(), val_type); }
                    Exp::Dot(obj, field, _) => {
                        check_exp(obj, &current_env, props, errors, context);
                        if !props.contains_key(field) && field != "x" && field != "y" {
                            //allow dynamic creation of properties but warn in case it's a typo
                            println!("Note: Dynamic property '{}' created on line {}.", field, line);                        }
                    }
                    _ => {}
                }
            }
            Command::If { condition, then_block, else_block, line: _ } => {
                check_bexp(condition, &current_env, props, errors, context);
                check_commands(then_block, &current_env, props, errors, context);
                if let Some(eb) = else_block { check_commands(eb, &current_env, props, errors, context); }
            }
            Command::While { condition, body, line: _ } => {
                check_bexp(condition, &current_env, props, errors, context);
                check_commands(body, &current_env, props, errors, context);
            }
            Command::Spawn { species: _, x, y, line: _ } => {
                check_exp(x, &current_env, props, errors, context);
                check_exp(y, &current_env, props, errors, context);
            }
            Command::Print(exps, _) => {
                for e in exps { check_exp(e, &current_env, props, errors, context); }
            }
            Command::Return(exp, _) => {
                check_exp(exp, &current_env, props, errors, context);
            }
            Command::Exp(exp, _) => {
                check_exp(exp, &current_env, props, errors, context);
            }
            Command::For { var, collection, body, line: _ } => {
                let mut for_env = current_env.clone();
                if collection == "environment" {
                    for_env.insert(var.clone(), Type::Object);

                }
                check_commands(body, &for_env, props, errors, context);
            }
        }
    }
}

fn check_exp(
    exp: &Exp,
    env: &HashMap<String, Type>,
    props: &HashMap<String, Type>,
    errors: &mut Vec<String>,
    context: &str,
) -> Type {
    match exp {
        Exp::Int(_, _) => Type::Int,
        Exp::StringLiteral(_, _) => Type::String,
        Exp::Bool(_, _) => Type::Bool,
        Exp::Var(name, line) => {
            if let Some(t) = env.get(name) {
                t.clone()
            } else if props.contains_key(name) {
                // for built in variables x, y, species, fitness
                props.get(name).unwrap().clone()
            } else {
                errors.push(format!("[{}] Undefined variable: {} at line {}", context, name, line));
                Type::Unknown
            }
        }
        Exp::BinaryOp(l, op, r, _) => {
            let lt = check_exp(l, env, props, errors, context);
            let rt = check_exp(r, env, props, errors, context);
            if op == "+" && (lt == Type::String || rt == Type::String) {
                Type::String
            } 
            else if op != "+" && (lt == Type::String || rt == Type::String) {
                errors.push(format!("Cannot use operator '{}' on a String", op));
                Type::Unknown
            }else {
                Type::Int
            }
        }
        Exp::Dot(obj, field, _) => {
            check_exp(obj, env, props, errors, context);
            props.get(field).cloned().unwrap_or(Type::Unknown)
        }
        Exp::Index(list, idx, _) => {
            check_exp(list, env, props, errors, context);
            check_exp(idx, env, props, errors, context);
            Type::Unknown
        }
        Exp::List(items, _) => {
            for i in items { check_exp(i, env, props, errors, context); }
            Type::List
        }
        Exp::Call(name, args, _) => {
            for a in args { check_exp(a, env, props, errors, context); }
            match name.as_str() {
                "random" | "len" | "dist" => Type::Int,
                "get_at" => Type::Object,
                _ => Type::Unknown,
            }
        }
    }
}

fn check_bexp(
    bexp: &BExp,
    env: &HashMap<String, Type>,
    props: &HashMap<String, Type>,
    errors: &mut Vec<String>,
    context: &str,
) {
    match bexp {
        BExp::Equal(l, r) | BExp::NotEqual(l, r) | BExp::Greater(l, r) | 
        BExp::Less(l, r) | BExp::GreaterEqual(l, r) | BExp::LessEqual(l, r) => {
            check_exp(l, env, props, errors, context);
            check_exp(r, env, props, errors, context); //2 exps
        }
        BExp::And(l, r) | BExp::Or(l, r) => {
            check_bexp(l, env, props, errors, context); //2 bexps
            check_bexp(r, env, props, errors, context);
        }
    }
}

fn infer_type(exp: &Exp) -> Type {
    match exp {
        Exp::Int(..) => Type::Int,
        Exp::StringLiteral(..) => Type::String,
        Exp::Bool(..) => Type::Bool,
        Exp::List(..) => Type::List,
        Exp::Call(name, _, _) if name == "get_at" => Type::Object,
        _ => Type::Unknown,
    }
}
