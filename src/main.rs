use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;
use std::sync::Arc;
use eframe::egui;

mod lexer;
mod parser;

use lexer::lexer;
use parser::Parser;

thread_local! {
    static GRID_CACHE: RefCell<Option<HashMap<(i32, i32), Rc<RefCell<Environment>>>>> = RefCell::new(None);
    static DRAW_COMMANDS: RefCell<Vec<DrawCmd>> = RefCell::new(Vec::new());
}

#[derive(Debug, Clone)]
pub enum DrawCmd {
    Rect { x: f32, y: f32, w: f32, h: f32, r: u8, g: u8, b: u8 },
    Line { x1: f32, y1: f32, x2: f32, y2: f32, r: u8, g: u8, b: u8, thickness: f32 },
    Circle { x: f32, y: f32, radius: f32, r: u8, g: u8, b: u8 },
}

// =============================================================
// The Sigma Environment (Recursive Scoping)
// =============================================================
#[derive(Debug, Clone)]
pub enum Value {
    Int(i32),
    Bool(bool),
    String(String),
    Object(Rc<RefCell<Environment>>),
    List(Vec<Value>),
}

#[derive(Debug, Clone)]
pub struct Environment {
    pub parent: Option<Rc<RefCell<Environment>>>,
    pub store: HashMap<String, Value>,
}

impl Environment {
    pub fn new(parent: Option<Rc<RefCell<Environment>>>) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self { parent, store: HashMap::new() }))
    }

    pub fn get(&self, name: &str) -> Value {
        if let Some(v) = self.store.get(name) { v.clone() }
        else if let Some(ref p) = self.parent { p.borrow().get(name) }
        else { Value::Int(0) }
    }
}

// =============================================================
// Top-Level Program Definitions
// =============================================================
#[derive(Debug, Clone, Default)]
pub struct EnvDef {
    pub width: i32,
    pub height: i32,
    pub steps: i32,
    pub visualize: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Program {
    pub env_width: i32,
    pub env_height: i32,
    pub env_steps: i32,
    pub visualize: bool,
    pub routines_block: HashMap<String, RoutineDef>,
    pub species_block: HashMap<String, SpeciesDef>,
    pub spawns_block: Vec<Command>,
    pub mutations_block: Vec<MutationRule>,
    pub fitness_block: Option<FitnessDef>,
    pub evolve_block: EvolveSettings,
    pub result_block: Vec<Command>,
    pub visualize_block: Vec<Command>,
}

#[derive(Debug, Clone, Default)]
pub struct EvolveSettings {
    pub generations: i32,
    pub stop_condition: Option<Vec<Command>>,
    pub instances: i32,
}

#[derive(Debug, Clone, Default)]
pub struct FitnessDef { 
    pub expressions: HashMap<String, Exp>,
    pub commands: Vec<Command>,
}

#[derive(Debug, Clone)]
pub struct RoutineDef { pub name: String, pub args: Vec<String>, pub body: Vec<Command> }

#[derive(Debug, Clone)]
pub struct SpeciesDef { 
    pub name: String, 
    pub properties: HashMap<String, Exp>, 
    pub routine_call: String, 
    pub routine_arg: String 
}

#[derive(Debug, Clone)]
pub struct MutationRule { 
    pub probability: f32, 
    pub action: String,
    pub body: Option<Vec<Command>>,
}

// =============================================================
// AST Logic
// =============================================================
#[derive(Debug, Clone)]
pub enum Exp { 
    Int(i32), 
    StringLiteral(String), 
    Var(String), 
    Dot(Box<Exp>, String), 
    BinaryOp(Box<Exp>, String, Box<Exp>),
    Call(String, Vec<Exp>),
    Index(Box<Exp>, Box<Exp>),
    List(Vec<Exp>),
}

impl Exp {
    pub fn eval(&self, env: Rc<RefCell<Environment>>, individuals: &[Individual]) -> i32 {
        match self {
            Exp::Int(v) => *v,
            Exp::StringLiteral(_) => 0,
            Exp::Var(name) => {
                if let Value::Int(v) = env.borrow().get(name) { v } else { 0 }
            }
            Exp::Dot(obj_exp, field) => {
                let obj_val = obj_exp.eval_to_val(env, individuals);
                if let Value::Object(obj_env) = obj_val {
                    if let Value::Int(v) = obj_env.borrow().get(field) { v } else { 0 }
                } else { 0 }
            }
            Exp::BinaryOp(l, op, r) => {
                let lv = l.eval(env.clone(), individuals);
                let rv = r.eval(env, individuals);
                match op.as_str() {
                    "+" => lv + rv,
                    "-" => lv - rv,
                    "*" => lv * rv,
                    "/" => if rv != 0 { lv / rv } else { 0 },
                    "%" => if rv != 0 { lv % rv } else { 0 },
                    "==" => if lv == rv { 1 } else { 0 },
                    ">" => if lv > rv { 1 } else { 0 },
                    "<" => if lv < rv { 1 } else { 0 },
                    _ => 0,
                }
            }
            Exp::Call(name, args) => {
                match name.as_str() {
                    "random" => {
                        use rand::Rng;
                        if args.len() == 2 {
                            let min = args[0].eval(env.clone(), individuals);
                            let max = args[1].eval(env, individuals);
                            if max > min {
                                rand::thread_rng().gen_range(min..max)
                            } else { min }
                        } else { 0 }
                    }
                    _ => 0,
                }
            }
            Exp::Index(list_exp, idx_exp) => {
                let list_val = list_exp.eval_to_val(env.clone(), individuals);
                let idx = idx_exp.eval(env, individuals) as usize;
                if let Value::List(l) = list_val {
                    if idx < l.len() {
                        if let Value::Int(v) = l[idx] { v } else { 0 }
                    } else { 0 }
                } else { 0 }
            }
            _ => 0,
        }
    }

    fn eval_to_val(&self, env: Rc<RefCell<Environment>>, individuals: &[Individual]) -> Value {
        match self {
            Exp::StringLiteral(s) => Value::String(s.clone()),
            Exp::Var(name) => env.borrow().get(name),
            Exp::Dot(obj_exp, field) => {
                let obj_val = obj_exp.eval_to_val(env, individuals);
                if let Value::Object(obj_env) = obj_val {
                    obj_env.borrow().get(field)
                } else { Value::Int(0) }
            }
            Exp::List(exps) => {
                let vals = exps.iter().map(|e| e.eval_to_val(env.clone(), individuals)).collect();
                Value::List(vals)
            }
            Exp::Index(list_exp, idx_exp) => {
                let list_val = list_exp.eval_to_val(env.clone(), individuals);
                let idx = idx_exp.eval(env, individuals) as usize;
                if let Value::List(l) = list_val {
                    if idx < l.len() { l[idx].clone() } else { Value::Int(0) }
                } else { Value::Int(0) }
            }
            Exp::Call(name, args) => {
                if name == "len" && args.len() == 1 {
                    let list_val = args[0].eval_to_val(env.clone(), individuals);
                    if let Value::List(l) = list_val {
                        return Value::Int(l.len() as i32);
                    }
                    return Value::Int(0);
                }
                if name == "push" && args.len() == 2 {
                    let list_val = args[0].eval_to_val(env.clone(), individuals);
                    let val = args[1].eval_to_val(env.clone(), individuals);
                    if let Value::List(mut l) = list_val {
                        l.push(val);
                        match &args[0] {
                            Exp::Var(name) => { env.borrow_mut().store.insert(name.clone(), Value::List(l)); }
                            Exp::Dot(obj_exp, field) => {
                                let obj_val = obj_exp.eval_to_val(env.clone(), individuals);
                                if let Value::Object(obj_env) = obj_val {
                                    obj_env.borrow_mut().store.insert(field.clone(), Value::List(l));
                                }
                            }
                            _ => {}
                        }
                    }
                    return Value::Int(0);
                }
                if name == "pop" && args.len() == 1 {
                    let list_val = args[0].eval_to_val(env.clone(), individuals);
                    if let Value::List(mut l) = list_val {
                        if let Some(v) = l.pop() {
                            match &args[0] {
                                Exp::Var(name) => { env.borrow_mut().store.insert(name.clone(), Value::List(l)); }
                                Exp::Dot(obj_exp, field) => {
                                    let obj_val = obj_exp.eval_to_val(env.clone(), individuals);
                                    if let Value::Object(obj_env) = obj_val {
                                        obj_env.borrow_mut().store.insert(field.clone(), Value::List(l));
                                    }
                                }
                                _ => {}
                            }
                            return v;
                        }
                    }
                    return Value::Int(0);
                }
                if name == "get_at" && args.len() == 2 {
                    let x = args[0].eval(env.clone(), individuals);
                    let y = args[1].eval(env, individuals);
                    
                    let cached = GRID_CACHE.with(|cache| {
                        if let Some(ref map) = *cache.borrow() {
                            return map.get(&(x, y)).cloned();
                        }
                        None
                    });

                    if let Some(env_rc) = cached {
                        return Value::Object(env_rc);
                    }

                    for other in individuals {
                        let ox = if let Value::Int(v) = other.env.borrow().get("x") { v } else { -1 };
                        let oy = if let Value::Int(v) = other.env.borrow().get("y") { v } else { -1 };
                        if ox == x && oy == y {
                            return Value::Object(other.env.clone());
                        }
                    }
                    Value::Int(0)
                } else if name == "draw_rect" && args.len() >= 4 {
                    let x = args[0].eval(env.clone(), individuals) as f32;
                    let y = args[1].eval(env.clone(), individuals) as f32;
                    let w = args[2].eval(env.clone(), individuals) as f32;
                    let h = args[3].eval(env.clone(), individuals) as f32;
                    let r = if args.len() > 4 { args[4].eval(env.clone(), individuals) as u8 } else { 255 };
                    let g = if args.len() > 5 { args[5].eval(env.clone(), individuals) as u8 } else { 255 };
                    let b = if args.len() > 6 { args[6].eval(env.clone(), individuals) as u8 } else { 255 };
                    DRAW_COMMANDS.with(|cmds| {
                        cmds.borrow_mut().push(DrawCmd::Rect { x, y, w, h, r, g, b });
                    });
                    Value::Int(0)
                } else if name == "draw_line" && args.len() >= 4 {
                    let x1 = args[0].eval(env.clone(), individuals) as f32;
                    let y1 = args[1].eval(env.clone(), individuals) as f32;
                    let x2 = args[2].eval(env.clone(), individuals) as f32;
                    let y2 = args[3].eval(env.clone(), individuals) as f32;
                    let r = if args.len() > 4 { args[4].eval(env.clone(), individuals) as u8 } else { 255 };
                    let g = if args.len() > 5 { args[5].eval(env.clone(), individuals) as u8 } else { 255 };
                    let b = if args.len() > 6 { args[6].eval(env.clone(), individuals) as u8 } else { 255 };
                    let thickness = if args.len() > 7 { args[7].eval(env.clone(), individuals) as f32 } else { 1.0 };
                    DRAW_COMMANDS.with(|cmds| {
                        cmds.borrow_mut().push(DrawCmd::Line { x1, y1, x2, y2, r, g, b, thickness });
                    });
                    Value::Int(0)
                } else if name == "draw_circle" && args.len() >= 3 {
                    let x = args[0].eval(env.clone(), individuals) as f32;
                    let y = args[1].eval(env.clone(), individuals) as f32;
                    let radius = args[2].eval(env.clone(), individuals) as f32;
                    let r = if args.len() > 3 { args[3].eval(env.clone(), individuals) as u8 } else { 255 };
                    let g = if args.len() > 4 { args[4].eval(env.clone(), individuals) as u8 } else { 255 };
                    let b = if args.len() > 5 { args[5].eval(env.clone(), individuals) as u8 } else { 255 };
                    DRAW_COMMANDS.with(|cmds| {
                        cmds.borrow_mut().push(DrawCmd::Circle { x, y, radius, r, g, b });
                    });
                    Value::Int(0)
                } else {
                    Value::Int(self.eval(env, individuals))
                }
            }
            _ => Value::Int(self.eval(env, individuals)),
        }
    }
}

#[derive(Debug, Clone)]
pub enum BExp { 
    Equal(Exp, Exp), 
    NotEqual(Exp, Exp), 
    Greater(Exp, Exp), 
    Less(Exp, Exp),
    And(Box<BExp>, Box<BExp>),
    Or(Box<BExp>, Box<BExp>)
}

impl BExp {
    pub fn eval(&self, env: Rc<RefCell<Environment>>, individuals: &[Individual]) -> bool {
        match self {
            BExp::And(l, r) => l.eval(env.clone(), individuals) && r.eval(env.clone(), individuals),
            BExp::Or(l, r) => l.eval(env.clone(), individuals) || r.eval(env.clone(), individuals),
            _ => {
                let lv = match self {
                    BExp::Equal(l, _) | BExp::NotEqual(l, _) | BExp::Greater(l, _) | BExp::Less(l, _) => l.eval_to_val(env.clone(), individuals),
                    _ => unreachable!(),
                };
                let rv = match self {
                    BExp::Equal(_, r) | BExp::NotEqual(_, r) | BExp::Greater(_, r) | BExp::Less(_, r) => r.eval_to_val(env.clone(), individuals),
                    _ => unreachable!(),
                };

                match self {
                    BExp::Equal(_, _) => match (lv, rv) {
                        (Value::Int(l), Value::Int(r)) => l == r,
                        (Value::Object(l), Value::Object(r)) => Rc::ptr_eq(&l, &r),
                        (Value::Int(0), Value::Object(_)) | (Value::Object(_), Value::Int(0)) => false,
                        _ => false,
                    },
                    BExp::NotEqual(_, _) => match (lv, rv) {
                        (Value::Int(l), Value::Int(r)) => l != r,
                        (Value::Object(l), Value::Object(r)) => !Rc::ptr_eq(&l, &r),
                        (Value::Int(0), Value::Object(_)) | (Value::Object(_), Value::Int(0)) => true,
                        _ => true,
                    },
                    BExp::Greater(_, _) => lv.as_int() > rv.as_int(),
                    BExp::Less(_, _) => lv.as_int() < rv.as_int(),
                    _ => unreachable!(),
                }
            }
        }
    }
}

impl Value {
    pub fn as_int(&self) -> i32 {
        match self {
            Value::Int(v) => *v,
            Value::Bool(b) => if *b { 1 } else { 0 },
            _ => 0,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Command {
    Assign { target: Exp, value: Exp },
    If { condition: BExp, then_block: Vec<Command>, else_block: Option<Vec<Command>> },
    While { condition: BExp, body: Vec<Command> },
    For { var: String, collection: String, body: Vec<Command> },
    Return(Exp),
    Print(Vec<Exp>),
    Spawn { species: String, x: Exp, y: Exp },
    Exp(Exp),
}

impl Command {
    pub fn execute(&self, env: Rc<RefCell<Environment>>, individuals: &[Individual], spawner: &mut Vec<Individual>, program: &Program) -> Option<Value> {
        match self {
            Command::Exp(exp) => {
                exp.eval_to_val(env, individuals);
                None
            }
            Command::Spawn { species, x, y } => {
                if let Some(species_def) = program.species_block.get(species) {
                    let xv = x.eval(env.clone(), individuals);
                    let yv = y.eval(env.clone(), individuals);
                    let new_env = Environment::new(None);
                    new_env.borrow_mut().store.insert("self".into(), Value::Object(new_env.clone()));
                    new_env.borrow_mut().store.insert("x".into(), Value::Int(xv));
                    new_env.borrow_mut().store.insert("y".into(), Value::Int(yv));
                    new_env.borrow_mut().store.insert("species".into(), Value::String(species.clone()));
                    
                    for (prop, exp) in &species_def.properties {
                        let val = exp.eval_to_val(new_env.clone(), individuals);
                        new_env.borrow_mut().store.insert(prop.clone(), val);
                    }
                    spawner.push(Individual { species: species.clone(), env: new_env, alive: true });
                }
                None
            }
            Command::Print(exps) => {
                let mut output = String::new();
                for exp in exps {
                    let val = exp.eval_to_val(env.clone(), individuals);
                    match val {
                        Value::Int(v) => output.push_str(&v.to_string()),
                        Value::String(s) => output.push_str(&s),
                        Value::Bool(b) => output.push_str(&b.to_string()),
                        Value::Object(_) => output.push_str("[Object]"),
                        Value::List(l) => output.push_str(&format!("{:?}", l)),
                    }
                    output.push(' ');
                }
                println!("{}", output.trim());
                None
            }
            Command::Assign { target, value } => {
                let val = value.eval_to_val(env.clone(), individuals);
                match target {
                    Exp::Var(name) => {
                        env.borrow_mut().store.insert(name.clone(), val);
                    }
                    Exp::Dot(obj_exp, field) => {
                        let obj_val = obj_exp.eval_to_val(env, individuals);
                        if let Value::Object(obj_env) = obj_val {
                            obj_env.borrow_mut().store.insert(field.clone(), val);
                        }
                    }
                    Exp::Index(list_exp, idx_exp) => {
                        let list_val = list_exp.eval_to_val(env.clone(), individuals);
                        let idx = idx_exp.eval(env.clone(), individuals) as usize;
                        if let Value::List(mut l) = list_val {
                            if idx < l.len() {
                                l[idx] = val;
                                match &**list_exp {
                                    Exp::Var(name) => {
                                        env.borrow_mut().store.insert(name.clone(), Value::List(l));
                                    }
                                    Exp::Dot(obj_exp, field) => {
                                        let obj_val = obj_exp.eval_to_val(env.clone(), individuals);
                                        if let Value::Object(obj_env) = obj_val {
                                            obj_env.borrow_mut().store.insert(field.clone(), Value::List(l));
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    _ => {}
                }
                None
            }
            Command::If { condition, then_block, else_block } => {
                if condition.eval(env.clone(), individuals) {
                    for cmd in then_block {
                        if let Some(v) = cmd.execute(env.clone(), individuals, spawner, program) { return Some(v); }
                    }
                } else if let Some(else_b) = else_block {
                    for cmd in else_b {
                        if let Some(v) = cmd.execute(env.clone(), individuals, spawner, program) { return Some(v); }
                    }
                }
                None
            }
            Command::While { condition, body } => {
                while condition.eval(env.clone(), individuals) {
                    for cmd in body {
                        if let Some(v) = cmd.execute(env.clone(), individuals, spawner, program) { return Some(v); }
                    }
                }
                None
            }
            Command::For { var, collection, body } => {
                if collection == "environment" {
                    for other in individuals {
                        if !other.alive { continue; }
                        env.borrow_mut().store.insert(var.clone(), Value::Object(other.env.clone()));
                        for cmd in body {
                            if let Some(v) = cmd.execute(env.clone(), individuals, spawner, program) { return Some(v); }
                        }
                    }
                }
                None
            }
            Command::Return(exp) => {
                Some(exp.eval_to_val(env, individuals))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Individual {
    pub species: String,
    pub env: Rc<RefCell<Environment>>,
    pub alive: bool,
}

#[derive(Debug, Clone)]
pub struct GenerationSnapshot {
    pub generation: i32,
    pub avg_fitness: i32,
    pub best_fitness: i32,
    pub individuals: Vec<Individual>,
}

pub struct World {
    pub width: i32,
    pub height: i32,
    pub individuals: Vec<Individual>,
    pub program: Arc<Program>,
    pub generation: i32,
    pub id: i32,
    pub fitness: i32,
}

impl World {
    pub fn new(program: Arc<Program>, id: i32) -> Self {
        Self {
            width: program.env_width,
            height: program.env_height,
            individuals: Vec::new(),
            program,
            generation: 0,
            id,
            fitness: 0,
        }
    }

    pub fn spawn(&mut self) {
        let mut spawner = Vec::new();
        let env = Environment::new(None);
        for cmd in &self.program.spawns_block {
            cmd.execute(env.clone(), &self.individuals, &mut spawner, &self.program);
        }
        self.individuals.extend(spawner);
    }

    pub fn step(&mut self) {
        let mut grid_map = HashMap::new();
        for ind in &self.individuals {
            let env_b = ind.env.borrow();
            let ix = if let Value::Int(v) = env_b.get("x") { v } else { -1 };
            let iy = if let Value::Int(v) = env_b.get("y") { v } else { -1 };
            if ix != -1 { grid_map.insert((ix, iy), ind.env.clone()); }
        }
        GRID_CACHE.with(|cache| *cache.borrow_mut() = Some(grid_map));

        let individuals_snapshot = self.individuals.clone();
        let mut spawner = Vec::new();
        for ind in &mut self.individuals {
            if !ind.alive { continue; }
            
            if let Some(species_def) = self.program.species_block.get(&ind.species) {
                if let Some(routine) = self.program.routines_block.get(&species_def.routine_call) {
                    let ind_env = ind.env.clone();
                    for cmd in &routine.body {
                        cmd.execute(ind_env.clone(), &individuals_snapshot, &mut spawner, &self.program);
                    }
                }
            }
        }
        self.individuals.extend(spawner);
        GRID_CACHE.with(|cache| *cache.borrow_mut() = None);
    }

    pub fn calculate_fitness(&self, ind: &Individual) -> i32 {
        if let Some(ref fitness_def) = self.program.fitness_block {
            if !fitness_def.commands.is_empty() {
                let env = Environment::new(None);
                env.borrow_mut().store.insert("self".into(), Value::Object(ind.env.clone()));
                let mut spawner = Vec::new();
                for cmd in &fitness_def.commands {
                    if let Some(val) = cmd.execute(env.clone(), &self.individuals, &mut spawner, &self.program) {
                        if let Value::Int(v) = val { return v; }
                    }
                }
            }
            if let Some(score_exp) = fitness_def.expressions.get("score") {
                return score_exp.eval(ind.env.clone(), &self.individuals);
            }
        }
        0
    }

    pub fn calculate_total_fitness(&mut self) -> i32 {
        let mut grid_map = HashMap::new();
        for ind in &self.individuals {
            let env_b = ind.env.borrow();
            let ix = if let Value::Int(v) = env_b.get("x") { v } else { -1 };
            let iy = if let Value::Int(v) = env_b.get("y") { v } else { -1 };
            if ix != -1 { grid_map.insert((ix, iy), ind.env.clone()); }
        }
        GRID_CACHE.with(|cache| *cache.borrow_mut() = Some(grid_map));

        let mut total = 0;
        let individuals_snapshot = self.individuals.clone();
        for i in 0..self.individuals.len() {
            let score = self.calculate_fitness(&individuals_snapshot[i]);
            self.individuals[i].env.borrow_mut().store.insert("fitness".into(), Value::Int(score));
            if score > total { total = score; }
        }
        self.fitness = total;
        GRID_CACHE.with(|cache| *cache.borrow_mut() = None);
        total
    }

    pub fn mutate(&mut self) {
        let mut spawner = Vec::new();
        let individuals_snapshot = self.individuals.clone();
        
        for ind in &mut self.individuals {
            if let Some(rule) = self.program.mutations_block.iter().find(|r| r.action == "mutation") {
                if rand::random::<f32>() < rule.probability {
                    if let Some(body) = &rule.body {
                        let mutation_env = Environment::new(None);
                        mutation_env.borrow_mut().store.insert("self".into(), Value::Object(ind.env.clone()));
                        for cmd in body {
                            cmd.execute(mutation_env.clone(), &individuals_snapshot, &mut spawner, &self.program);
                        }
                    }
                }
            }
        }
    }
}

// =============================================================
// Static Semantic Pass
// =============================================================

fn validate_program(prog: &Program) -> Result<(), String> {
    let globals = vec!["width", "height", "steps", "environment", "self"];

    // 1. Validate Routines
    for (species_name, species) in &prog.species_block {
        let mut valid_props = species.properties.keys().cloned().collect::<Vec<String>>();
        valid_props.extend(vec!["x".into(), "y".into(), "species".into(), "fitness".into()]);

        if !species.routine_call.is_empty() {
            if let Some(routine) = prog.routines_block.get(&species.routine_call) {
                let mut locals = Vec::new();
                for arg in &routine.args { locals.push(arg.clone()); }
                check_block_semantics(&routine.body, &globals, &valid_props, &mut locals)?;
            } else {
                return Err(format!("Species '{}' calls undefined routine '{}'", species_name, species.routine_call));
            }
        }
    }

    // 2. Validate SPAWN block
    let mut spawn_locals = Vec::new();
    check_block_semantics(&prog.spawns_block, &globals, &[], &mut spawn_locals)?;

    // 3. Validate FITNESS block
    if let Some(fitness) = &prog.fitness_block {
        let mut fitness_locals = Vec::new();
        fitness_locals.push("self".into());
        let builtin_props = vec!["x".into(), "y".into(), "species".into(), "fitness".into()];
        check_block_semantics(&fitness.commands, &globals, &builtin_props, &mut fitness_locals)?;
        for exp in fitness.expressions.values() {
            check_expression_semantics(exp, &globals, &builtin_props, &fitness_locals)?;
        }
    }

    Ok(())
}

fn check_block_semantics(cmds: &[Command], globals: &[&str], props: &[String], locals: &mut Vec<String>) -> Result<(), String> {
    for cmd in cmds {
        match cmd {
            Command::Assign { target, value } => {
                check_expression_semantics(value, globals, props, locals)?;
                if let Exp::Var(name) = target {
                    if !globals.contains(&name.as_str()) && !props.contains(name) && !locals.contains(name) {
                        locals.push(name.clone());
                    }
                } else if let Exp::Dot(obj, field) = target {
                    check_expression_semantics(obj, globals, props, locals)?;
                    if let Exp::Var(name) = &**obj {
                        if name == "self" {
                            if !props.contains(field) {
                                return Err(format!("Semantic Error: Species property '{}' is not defined!", field));
                            }
                        }
                    }
                } else if let Exp::Index(list, idx) = target {
                    check_expression_semantics(list, globals, props, locals)?;
                    check_expression_semantics(idx, globals, props, locals)?;
                }
            }
            Command::If { condition, then_block, else_block } => {
                check_bexp_semantics(condition, globals, props, locals)?;
                let mut then_locals = locals.clone();
                check_block_semantics(then_block, globals, props, &mut then_locals)?;
                if let Some(eb) = else_block {
                    let mut else_locals = locals.clone();
                    check_block_semantics(eb, globals, props, &mut else_locals)?;
                }
            }
            Command::While { condition, body } => {
                check_bexp_semantics(condition, globals, props, locals)?;
                let mut body_locals = locals.clone();
                check_block_semantics(body, globals, props, &mut body_locals)?;
            }
            Command::For { var, collection, body } => {
                if !globals.contains(&collection.as_str()) && !locals.contains(collection) {
                    return Err(format!("Undefined collection '{}' in for loop", collection));
                }
                let mut body_locals = locals.clone();
                body_locals.push(var.clone());
                check_block_semantics(body, globals, props, &mut body_locals)?;
            }
            Command::Spawn { species: _, x, y } => {
                check_expression_semantics(x, globals, props, locals)?;
                check_expression_semantics(y, globals, props, locals)?;
            }
            Command::Print(exps) => {
                for e in exps { check_expression_semantics(e, globals, props, locals)?; }
            }
            Command::Return(e) => { check_expression_semantics(e, globals, props, locals)?; }
            Command::Exp(e) => { check_expression_semantics(e, globals, props, locals)?; }
        }
    }
    Ok(())
}

fn check_expression_semantics(exp: &Exp, globals: &[&str], props: &[String], locals: &[String]) -> Result<(), String> {
    match exp {
        Exp::Var(name) => {
            if !globals.contains(&name.as_str()) && !props.contains(name) && !locals.contains(name) {
                // Built-in functions or constants might be here, but we check variables
                let builtins = vec!["random", "len", "push", "pop", "get_at", "draw_rect", "draw_line", "draw_circle"];
                if !builtins.contains(&name.as_str()) {
                    return Err(format!("Semantic Error: Variable '{}' is not defined!", name));
                }
            }
        }
        Exp::BinaryOp(l, _, r) => {
            check_expression_semantics(l, globals, props, locals)?;
            check_expression_semantics(r, globals, props, locals)?;
        }
        Exp::Dot(obj, field) => {
            check_expression_semantics(obj, globals, props, locals)?;
            if let Exp::Var(name) = &**obj {
                if name == "self" {
                    if !props.contains(field) {
                        return Err(format!("Semantic Error: Species property '{}' is not defined!", field));
                    }
                }
            }
        }
        Exp::Call(_, args) => {
            for a in args { check_expression_semantics(a, globals, props, locals)?; }
        }
        Exp::Index(list, idx) => {
            check_expression_semantics(list, globals, props, locals)?;
            check_expression_semantics(idx, globals, props, locals)?;
        }
        Exp::List(exps) => {
            for e in exps { check_expression_semantics(e, globals, props, locals)?; }
        }
        _ => {}
    }
    Ok(())
}

fn check_bexp_semantics(bexp: &BExp, globals: &[&str], props: &[String], locals: &[String]) -> Result<(), String> {
    match bexp {
        BExp::Equal(l, r) | BExp::NotEqual(l, r) | BExp::Greater(l, r) | BExp::Less(l, r) => {
            check_expression_semantics(l, globals, props, locals)?;
            check_expression_semantics(r, globals, props, locals)?;
        }
        BExp::And(l, r) | BExp::Or(l, r) => {
            check_bexp_semantics(l, globals, props, locals)?;
            check_bexp_semantics(r, globals, props, locals)?;
        }
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("Usage: simulanka <file.txt> [generations] [instances]");
        return;
    }

    let input = std::fs::read_to_string(&args[1]).expect("Could not read file");
    let tokens = lexer(&input);
    let mut parser = Parser::new(tokens);
    
    match parser.parse_program() {
        Ok(prog) => {
            if let Err(e) = validate_program(&prog) {
                println!("Semantic Error: {}", e);
                return;
            }
            let prog_arc = Arc::new(prog);
            let num_generations = if args.len() > 2 { args[2].parse().unwrap_or(prog_arc.evolve_block.generations) } else { prog_arc.evolve_block.generations };
            let num_instances = if args.len() > 3 { args[3].parse().unwrap_or(prog_arc.evolve_block.instances) } else { prog_arc.evolve_block.instances };
            
            let mut instances: Vec<World> = (0..num_instances)
                .map(|i| {
                    let mut w = World::new(prog_arc.clone(), i);
                    w.spawn();
                    w
                })
                .collect();

            let mut history = Vec::new();

            if prog_arc.visualize {
                let options = eframe::NativeOptions::default();
                let width = prog_arc.env_width;
                let height = prog_arc.env_height;

                let _ = eframe::run_native(
                    "Simulanka Maze Evolution",
                    options,
                    Box::new(move |_cc| {
                        Ok(Box::new(SimApp {
                            instances,
                            history,
                            current_gen_idx: 0,
                            world_width: width,
                            world_height: height,
                            prog_arc,
                            num_generations,
                            num_instances,
                            current_g: 0,
                            running: false,
                            global_best_fitness: 0,
                        }))
                    }),
                );
            }
        }
        Err(e) => println!("Error: {}", e),
    }
}

struct SimApp {
    instances: Vec<World>,
    history: Vec<GenerationSnapshot>,
    current_gen_idx: usize,
    world_width: i32,
    world_height: i32,
    prog_arc: Arc<Program>,
    num_generations: i32,
    num_instances: i32,
    current_g: i32,
    running: bool,
    global_best_fitness: i32,
}

impl SimApp {
    fn run_generation(&mut self) {
        let start = std::time::Instant::now();
        if self.current_g >= self.num_generations {
            self.running = false;
            return;
        }
        self.current_g += 1;
        let g = self.current_g;

        for world in &mut self.instances {
            world.generation = g;
            for _ in 0..self.prog_arc.env_steps {
                world.step();
            }
            world.calculate_total_fitness();
        }

        self.instances.sort_by_key(|w| -w.fitness);

        let avg_fitness = self.instances.iter().map(|w| w.fitness).sum::<i32>() / self.num_instances.max(1);
        let best_fitness = self.instances[0].fitness;
        if best_fitness > self.global_best_fitness {
            self.global_best_fitness = best_fitness;
        }

        let best_world = &self.instances[0];
        let snapshot = GenerationSnapshot {
            generation: g,
            avg_fitness,
            best_fitness,
            individuals: best_world.individuals.iter().map(|ind| {
                let new_env = Environment::new(None);
                new_env.borrow_mut().store = ind.env.borrow().store.clone();
                new_env.borrow_mut().store.insert("self".into(), Value::Object(new_env.clone()));
                Individual {
                    species: ind.species.clone(),
                    env: new_env,
                    alive: ind.alive,
                }
            }).collect(),
        };
        let is_at_end = self.current_gen_idx == self.history.len().saturating_sub(1);
        self.history.push(snapshot);
        if is_at_end {
            self.current_gen_idx = self.history.len() - 1;
        }

        let duration = start.elapsed();
        println!("[Gen {}] Avg: {}, Best: {} (took {:?})", g, avg_fitness, best_fitness, duration);

        let keep_count = (self.num_instances / 2).max(1) as usize;
        let mut next_gen = Vec::new();
        
        for i in 0..self.num_instances as usize {
            let parent_idx = i % keep_count;
            let mut child = World::new(self.prog_arc.clone(), i as i32);
            child.generation = g;
            for ind in &self.instances[parent_idx].individuals {
                let child_env = Environment::new(None);
                child_env.borrow_mut().store = ind.env.borrow().store.clone();
                child_env.borrow_mut().store.insert("self".into(), Value::Object(child_env.clone()));
                child.individuals.push(Individual {
                    species: ind.species.clone(),
                    env: child_env,
                    alive: true,
                });
            }
            
            if i >= keep_count {
                let p2_idx = (i + 1) % keep_count;
                let p2 = &self.instances[p2_idx];
                if let Some(rule) = self.prog_arc.mutations_block.iter().find(|r| r.action == "crossover") {
                    if let Some(body) = &rule.body {
                        for j in 0..child.individuals.len() {
                            let crossover_env = Environment::new(None);
                            crossover_env.borrow_mut().store.insert("parent1".into(), Value::Object(child.individuals[j].env.clone()));
                            crossover_env.borrow_mut().store.insert("parent2".into(), Value::Object(p2.individuals[j].env.clone()));
                            crossover_env.borrow_mut().store.insert("child".into(), Value::Object(child.individuals[j].env.clone()));
                            
                            let mut spawner = Vec::new();
                            for cmd in body {
                                cmd.execute(crossover_env.clone(), &[], &mut spawner, &self.prog_arc);
                            }
                        }
                    }
                }
            }

            child.mutate();
            next_gen.push(child);
        }
        self.instances = next_gen;
    }
}

impl eframe::App for SimApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.running {
            self.run_generation();
            ctx.request_repaint();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.running {
                ui.ctx().request_repaint();
            }
            ui.heading("Simulanka Maze Evolution");

            ui.horizontal(|ui| {
                if ui.button(if self.running { "Stop" } else { "Start" }).clicked() {
                    self.running = !self.running;
                }
                if ui.button("Step").clicked() {
                    self.run_generation();
                }
                if ui.button("Reset").clicked() {
                    self.current_g = 0;
                    self.history.clear();
                    self.current_gen_idx = 0;
                    self.running = false;
                    self.global_best_fitness = 0;
                    self.instances = (0..self.num_instances)
                        .map(|i| {
                            let mut w = World::new(self.prog_arc.clone(), i);
                            w.spawn();
                            w
                        })
                        .collect();
                }
                if ui.button("<- Previous").clicked() && self.current_gen_idx > 0 {
                    self.current_gen_idx -= 1;
                    self.running = false;
                }
                ui.label(format!("Gen {} / {}", self.current_gen_idx + 1, self.history.len()));
                if ui.button("Next ->").clicked() && self.current_gen_idx < self.history.len() - 1 {
                    self.current_gen_idx += 1;
                    self.running = false;
                }
            });

            if self.history.is_empty() {
                ui.label("Press Start or Step to begin evolution.");
                return;
            }

            let snapshot = &self.history[self.current_gen_idx];
            ui.label(format!("Avg Fitness: {}, Gen Best: {}, Global Best: {}", snapshot.avg_fitness, snapshot.best_fitness, self.global_best_fitness));
            
            if ui.add(egui::Slider::new(&mut self.current_gen_idx, 0..=self.history.len().saturating_sub(1)).text("View Gen")).changed() {
                self.running = false;
            }

            ui.separator();

            // Clear previous draw commands
            DRAW_COMMANDS.with(|cmds| cmds.borrow_mut().clear());

            // Execute VISUALIZE block
            if !self.prog_arc.visualize_block.is_empty() {
                let viz_env = Environment::new(None);
                viz_env.borrow_mut().store.insert("width".into(), Value::Int(self.world_width));
                viz_env.borrow_mut().store.insert("height".into(), Value::Int(self.world_height));
                
                // Set up grid cache for VISUALIZE block
                let mut grid_map = HashMap::new();
                for ind in &snapshot.individuals {
                    let env_b = ind.env.borrow();
                    let ix = if let Value::Int(v) = env_b.get("x") { v } else { -1 };
                    let iy = if let Value::Int(v) = env_b.get("y") { v } else { -1 };
                    if ix != -1 { grid_map.insert((ix, iy), ind.env.clone()); }
                }
                GRID_CACHE.with(|cache| *cache.borrow_mut() = Some(grid_map));

                let mut spawner = Vec::new();
                for cmd in &self.prog_arc.visualize_block {
                    cmd.execute(viz_env.clone(), &snapshot.individuals, &mut spawner, &self.prog_arc);
                }

                GRID_CACHE.with(|cache| *cache.borrow_mut() = None);
            }

            let size = 600.0;
            let (rect, _response) = ui.allocate_at_least(egui::vec2(size, size), egui::Sense::hover());
            let painter = ui.painter();
            
            painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(20, 20, 20));
            
            DRAW_COMMANDS.with(|cmds| {
                for cmd in cmds.borrow().iter() {
                    match cmd {
                        DrawCmd::Rect { x, y, w, h, r, g, b } => {
                            let min = rect.min + egui::vec2(*x, *y);
                            painter.rect_filled(
                                egui::Rect::from_min_size(min, egui::vec2(*w, *h)),
                                0.0,
                                egui::Color32::from_rgb(*r, *g, *b)
                            );
                        }
                        DrawCmd::Line { x1, y1, x2, y2, r, g, b, thickness } => {
                            painter.line_segment(
                                [rect.min + egui::vec2(*x1, *y1), rect.min + egui::vec2(*x2, *y2)],
                                egui::Stroke::new(*thickness, egui::Color32::from_rgb(*r, *g, *b))
                            );
                        }
                        DrawCmd::Circle { x, y, radius, r, g, b } => {
                            painter.circle_filled(
                                rect.min + egui::vec2(*x, *y),
                                *radius,
                                egui::Color32::from_rgb(*r, *g, *b)
                            );
                        }
                    }
                }
            });
        });
    }
}
