//types.rs - core data types for simulanka
//this file contains all the basic data structures used throughout
//the interpreter. no complex logic here - just definitions.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

//environment - stores variables for each individual/scope
//think of this like a "box" that holds named values.
//each creature (individual) has its own environment.

#[derive(Debug)]
pub struct Environment {
    pub store: HashMap<String, Value>,
}

impl Environment {
    pub fn new() -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self { store: HashMap::new() }))
    }

    pub fn deep_copy_store(&self) -> HashMap<String, Value> {
        //1. initialize the new container
        let mut new_store = HashMap::new();

        //2. explicitly loop through the current store
        for (key, value) in self.store.iter() {
            //3. perform the copies step-by-step
            let cloned_key = key.clone();
            let deep_copied_value = value.deep_copy();

            //4. insert into the new container
            new_store.insert(cloned_key, deep_copied_value);
        }

        //5. return the finished product
        new_store
    }
}

//value - what variables can hold
//a value is anything that can be stored in a variable:
//numbers, text, lists, or references to other environments.

#[derive(Debug, Clone)]
pub enum Value {
    Int(i32),                           //a whole number: 42
    Bool(bool),                         //true or false
    String(String),                     //text: "hello"
    Object(Arc<RwLock<Environment>>),   //reference to another creature
    List(Arc<RwLock<Vec<Value>>>),      //a list of values: [1, 2, 3]
    Environment,                        //the global environment grid
    GridRow(i32),                       //a row in the grid (for environment[x][y])
}

impl Value {
    //convert any value to an integer (for math operations)
    pub fn to_int(&self) -> i32 {
        match self {
            Value::Int(v) => *v,
            Value::Bool(b) => if *b { 1 } else { 0 },
            Value::String(s) => s.parse().unwrap_or(0),
            _ => 0,
        }
    }

    //convert any value to a string for printing
    pub fn to_string(&self) -> String {
        match self {
            Value::Int(v) => v.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::String(s) => s.clone(),
            Value::Object(_) => "[Object]".to_string(),
            Value::List(l) => format!("{:?}", l.read().unwrap()),
            Value::Environment => "[Environment]".to_string(),
            Value::GridRow(x) => format!("[GridRow {}]", x),
        }
    }

    //create a deep copy (especially important for lists)
    pub fn deep_copy(&self) -> Value {
        match self {
            Value::List(list) => {
                //Determine the new vector of values imperatively
                let mut new_vec = Vec::new();
                for v in list.read().unwrap().iter() {
                    new_vec.push(v.deep_copy());
                }
                Value::List(Arc::new(RwLock::new(new_vec)))
            }
            _ => self.clone(),
        }
    }
}

//individual - a single creature in the simulation
//each creature has a species name and its own environment.

#[derive(Debug, Clone)]
pub struct Individual {
    pub species: String,                //what species is this? e.g., "ant"
    pub env: Arc<RwLock<Environment>>,  //its personal data (x, y, energy, etc.)
}

impl Individual {
    pub fn deep_clone(&self) -> Self {
        let new_env = Environment::new();
        let old_ptr = self.env.clone();
        let mut new_store = self.env.read().unwrap().deep_copy_store();
        
        fn fix(v: &mut Value, old: &Arc<RwLock<Environment>>, new: &Arc<RwLock<Environment>>) {
            match v {
                Value::Object(obj) if Arc::ptr_eq(obj, old) => *v = Value::Object(new.clone()),
                Value::List(l) => { for i in l.write().unwrap().iter_mut() { fix(i, old, new); } }
                _ => {}
            }
        }

        for v in new_store.values_mut() { fix(v, &old_ptr, &new_env); }
        new_env.write().unwrap().store = new_store;
        Self { species: self.species.clone(), env: new_env }
    }
}

//generation snapshot - state at a point in time
//used for visualization - stores the state of a generation
//so we can replay it later.

#[derive(Debug, Clone)]
pub struct GenerationSnapshot {
    pub avg_fitness: i32,
    pub best_fitness: i32,
    pub individuals: Vec<Individual>,
    pub step_history: Vec<Vec<Individual>>,
}

//standard exp + bexp setup
//exp

#[derive(Debug, Clone)]
pub enum Exp {
    Int(i32, usize),                                 //literal number: 42
    Bool(bool, usize),                               //literal boolean: true
    StringLiteral(String, usize),                    //literal text: "hello"
    Var(String, usize),                              //variable name: x
    Dot(Box<Exp>, String, usize),                    //field access: self.energy
    BinaryOp(Box<Exp>, String, Box<Exp>, usize),     //math: a + b
    Call(String, Vec<Exp>, usize),                   //function call: random(1, 10)
    Index(Box<Exp>, Box<Exp>, usize),                //array access: list[i]
    List(Vec<Exp>, usize),                           //list literal: [1, 2, 3]
}

impl Exp {
}

//bexp
#[derive(Debug, Clone)]
pub enum BExp {
    Equal(Exp, Exp),        //a == b
    NotEqual(Exp, Exp),     //a != b
    Greater(Exp, Exp),      //a > b
    Less(Exp, Exp),         //a < b
    GreaterEqual(Exp, Exp), //a >= b
    LessEqual(Exp, Exp),    //a <= b
    And(Box<BExp>, Box<BExp>), //cond1 && cond2
    Or(Box<BExp>, Box<BExp>),  //cond1 || cond2
}

//commands -> actions to perform

#[derive(Debug, Clone)]
pub enum Command {
    Assign { target: Exp, value: Exp, line: usize },
    If {
        condition: BExp,
        then_block: Vec<Command>,
        else_block: Option<Vec<Command>>,
        line: usize,
    },
    While {
        condition: BExp,
        body: Vec<Command>,
        line: usize,
    },
    For {
        var: String,
        collection: String,
        body: Vec<Command>,
        line: usize,
    },
    Return(Exp, usize),
    Print(Vec<Exp>, usize),
    Spawn { species: String, x: Exp, y: Exp, line: usize },
    Exp(Exp, usize),
}

impl Command {
}

//program structure - the parsed program

//definition of a species 
#[derive(Debug, Clone)]
pub struct SpeciesDef {
    pub properties: HashMap<String, Exp>,  //default property values
    pub routine_call: String,              //which routine to run each step
}

//a routine (kinda a function) definition
#[derive(Debug, Clone)]
pub struct RoutineDef {
    pub name: String,
    pub body: Vec<Command>,
}

//a mutation/crossover rule
#[derive(Debug, Clone)]
pub struct MutationRule {
    pub probability: f32,
    pub action: String, //name
    pub body: Option<Vec<Command>>, //commands
}

//settings for the evolutionary process
#[derive(Debug, Clone)]
pub struct EvolveBlock {
    pub generations: i32,
    pub instances: i32,
}

impl Default for EvolveBlock {
    fn default() -> Self {
        Self {
            generations: 1,
            instances: 1,
        }
    }
}

//fitness calculation definition
#[derive(Debug, Clone, Default)]
pub struct FitnessBlock {
    pub commands: Vec<Command>,
}

//the complete program
#[derive(Debug, Clone)]
pub struct Program {
    //environment settings
    pub env_width: i32,
    pub env_height: i32,
    pub env_steps: i32,
    
    //program blocks
    pub routines_block: HashMap<String, RoutineDef>,
    pub species_block: HashMap<String, SpeciesDef>,
    pub spawns_block: Vec<Command>,
    pub mutations_block: Vec<MutationRule>,
    pub fitness_block: FitnessBlock,
    pub evolve_block: EvolveBlock,
    pub visualize_block: Vec<Command>,
    pub visualize: bool,
}

impl Default for Program {
    fn default() -> Self {
        Self {
            env_width: 100,
            env_height: 100,
            env_steps: 100,
            routines_block: HashMap::new(),
            species_block: HashMap::new(),
            spawns_block: Vec::new(),
            mutations_block: Vec::new(),
            fitness_block: FitnessBlock::default(),
            evolve_block: EvolveBlock::default(),
            visualize_block: Vec::new(),
            visualize: true, // visualize by default
        }
    }
}

//draw commands -> for visualization

#[derive(Debug, Clone)]
pub enum DrawCmd {
    Rect { x: f32, y: f32, w: f32, h: f32, r: u8, g: u8, b: u8 },
    Line { x1: f32, y1: f32, x2: f32, y2: f32, r: u8, g: u8, b: u8, thickness: f32 },
    Circle { x: f32, y: f32, radius: f32, r: u8, g: u8, b: u8 },
}

//world - contains all individuals and simulation state

#[derive(Clone)]
pub struct World {
    pub width: i32,
    pub height: i32,
    pub individuals: Vec<Individual>,
    pub program: Arc<Program>,
    pub generation: i32,
    pub id: i32,
    pub fitness: i32,
    pub record_history: bool,
    pub history: Vec<Vec<Individual>>,
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
            record_history: false,
            history: Vec::new(),
        }
    }

    //move individuals out and create a new world with them
    pub fn take(&mut self) -> World {
        World {
            width: self.width,
            height: self.height,
            individuals: std::mem::take(&mut self.individuals),
            program: self.program.clone(),
            generation: self.generation,
            id: self.id,
            fitness: self.fitness,
            record_history: self.record_history,
            history: std::mem::take(&mut self.history),
        }
    }
}
