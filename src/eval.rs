//eval.rs - evaluates expressions and commands

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, RwLock}; //arc is really really really important - multithreading

use crate::types::*;

//global state (thread_local for safety)

thread_local! {
    //individuals are shared grid is cached for each thread
    pub static GRID_CACHE: RefCell<Option<HashMap<(i32, i32), Arc<RwLock<Environment>>>>> = RefCell::new(None);
    
    //drawing commands for visualization
    pub static DRAW_COMMANDS: RefCell<Vec<DrawCmd>> = RefCell::new(Vec::new());
    
    //current world size
    pub static WORLD_DIMENSIONS: RefCell<(i32, i32)> = RefCell::new((100, 100));
}

//exp evaluation

impl Exp {
    //convert an expression to an integer
    pub fn eval(&self, env: Arc<RwLock<Environment>>, individuals: &[Individual]) -> i32 {
        match self {
            //simple values
            Exp::Int(v, _l) => *v,
            Exp::Bool(b, _l) => if *b { 1 } else { 0 },
            
            //variable lookup - check local first then self
            Exp::Var(name, _l) => {
                let env_ref = env.read().unwrap();
                //first check local scope
                if let Some(v) = env_ref.store.get(name) {
                    return v.to_int();
                }
                //then check if we have a 'self' and look there
                if let Some(Value::Object(self_env)) = env_ref.store.get("self") {
                    return self_env.read().unwrap().store.get(name).map_or(0, |v| v.to_int());
                }
                0
            }
            
            //field access: self.x, target.speed
            Exp::Dot(obj, field, _l) => {
                let obj_val = obj.eval_to_val(env.clone(), individuals);
                if let Value::Object(obj_env) = obj_val {
                    obj_env.read().unwrap().store.get(field).map_or(0, |v| v.to_int())
                } else {
                    0
                }
            }
            
            //math operations: a + b, x * y
            Exp::BinaryOp(left, op, right, _l) => {
                let left_val = left.eval(env.clone(), individuals);
                let right_val = right.eval(env, individuals);
                
                match op.as_str() {
                    "+" => left_val + right_val,
                    "-" => left_val - right_val,
                    "*" => left_val * right_val,
                    "/" => if right_val != 0 { left_val / right_val } else { 0 },
                    "%" => if right_val != 0 { left_val % right_val } else { 0 },
                    _ => 0,
                }
            }
            
            //function calls: random(0, 10)
            Exp::Call(name, args, _l) => { //_l is not used so _
                //handle random separately since its used the most and doesnt depend on other objects
                if name == "random" && args.len() == 2 {
                    use rand::Rng;
                    let min = args[0].eval(env.clone(), individuals);
                    let max = args[1].eval(env, individuals);
                    if max > min {
                        rand::thread_rng().gen_range(min..max)
                    } else {
                        min
                    }
                } else {
                    //for other calls get full value and convert to int
                    self.eval_to_val(env, individuals).to_int()
                }
            }
            
            //array access: genes[i]
            Exp::Index(list_exp, idx_exp, _l) => {
                let list_val = list_exp.eval_to_val(env.clone(), individuals);
                let idx = idx_exp.eval(env, individuals) as usize;
                
                if let Value::List(list) = list_val {
                    let borrowed = list.read().unwrap();
                    if idx < borrowed.len() {
                        borrowed[idx].to_int()
                    } else {
                        0
                    }
                } else {
                    0
                }
            }
            
            _ => 0,
        }
    }

    //get the full value of an expression (keeps strings lists etc)
    pub fn eval_to_val(&self, env: Arc<RwLock<Environment>>, individuals: &[Individual]) -> Value {
        match self {
            //literals
            Exp::Int(v, _l) => Value::Int(*v),
            Exp::Bool(b, _l) => Value::Bool(*b),
            Exp::StringLiteral(s, _l) => Value::String(s.clone()),
            
            //variable lookup - high speed: flat access
            Exp::Var(name, _l) => {
                let env_ref = env.read().unwrap();
                //check local/creature store
                if let Some(v) = env_ref.store.get(name) {
                    return v.clone();
                }
                
                if name == "environment" {
                    return Value::Environment;
                }
                
                Value::Int(0)
            }
            
            //field access: self.species, target.x
            Exp::Dot(obj, field, _l) => {
                let obj_val = obj.eval_to_val(env.clone(), individuals);
                if let Value::Object(obj_env) = obj_val {
                    obj_env.read().unwrap().store.get(field).cloned().unwrap_or(Value::Int(0))
                } else {
                    Value::Int(0)
                }
            }
            
            //list literal: [1, 2, 3]
            Exp::List(items, _l) => {
                let mut values = Vec::new();
                for item in items {
                    values.push(item.eval_to_val(env.clone(), individuals));
                }
                Value::List(Arc::new(RwLock::new(values)))
            }
            
            //array/grid access
            Exp::Index(list_exp, idx_exp, _l) => {
                let list_val = list_exp.eval_to_val(env.clone(), individuals);
                let idx = idx_exp.eval(env.clone(), individuals);
                
                match list_val {
                    //normal list access: my_list[i]
                    Value::List(list) => {
                        let borrowed = list.read().unwrap();
                        let i = idx as usize;
                        if i < borrowed.len() {
                            borrowed[i].clone()
                        } else {
                            Value::Int(0)
                        }
                    }
                    //grid access: environment[x]
                    Value::Environment => Value::GridRow(idx),
                    //grid cell access: environment[x][y]
                    Value::GridRow(x) => {
                        let y = idx;
                        
                        //get world size for wrapping
                        let (width, height) = WORLD_DIMENSIONS.with(|d| *d.borrow());
                        let wrapped_x = ((x % width) + width) % width;
                        let wrapped_y = ((y % height) + height) % height;

                        //try cache first (faster)
                        let cached = GRID_CACHE.with(|cache| {
                            if let Option::Some(map) = cache.borrow().as_ref() {
                                map.get(&(wrapped_x, wrapped_y)).cloned()
                            } else {
                                None
                            }
                        });
                        
                        if let Some(found) = cached {
                            return Value::Object(found);
                        }

                        //search through individuals
                        for ind in individuals {
                            let env_b = ind.env.read().unwrap();
                            let store = &env_b.store;
                            let ind_x = store.get("x").map_or(0, |v| v.to_int());
                            let ind_y = store.get("y").map_or(0, |v| v.to_int());
                            if (ind_x % width + width) % width == wrapped_x && 
                               (ind_y % height + height) % height == wrapped_y {
                                return Value::Object(ind.env.clone());
                            }
                        }
                        Value::Int(0)
                    }
                    _ => Value::Int(0)
                }
            }
            
            //function calls
            Exp::Call(name, args, _l) => {
                self.run_builtin(name, args, env, individuals)
            }
            
            //for anything else, convert to int
            Exp::BinaryOp(_, _, _, _l) => Value::Int(self.eval(env, individuals)),
        }
    }

    //run a built-in function
    fn run_builtin(
        &self,
        name: &str,
        args: &[Exp],
        env: Arc<RwLock<Environment>>,
        individuals: &[Individual],
    ) -> Value {
        match name {
            //len(list) - get list length
            "len" => {
                if args.len() >= 1 {
                    if let Value::List(list) = args[0].eval_to_val(env, individuals) {
                        return Value::Int(list.read().unwrap().len() as i32);
                    }
                }
                Value::Int(0)
            }
            
            //push(list, value) - add to list
            "push" => {
                if args.len() >= 2 {
                    if let Value::List(list) = args[0].eval_to_val(env.clone(), individuals) {
                        let value = args[1].eval_to_val(env, individuals);
                        list.write().unwrap().push(value);
                    }
                }
                Value::Int(0)
            }
            
            //pop(list) - remove from list
            "pop" => {
                if args.len() >= 1 {
                    if let Value::List(list) = args[0].eval_to_val(env, individuals) {
                        return list.write().unwrap().pop().unwrap_or(Value::Int(0));
                    }
                }
                Value::Int(0)
            }
            
                    //get_at(nx, ny)
                    "get_at" => {
                        if args.len() >= 2 {
                            let x = args[0].eval(env.clone(), individuals);
                            let y = args[1].eval(env, individuals);
                            
                            //Try cache first
                            let cached = GRID_CACHE.with(|cache| {
                                if let Some(map) = cache.borrow().as_ref() {
                                    map.get(&(x, y)).cloned()
                                } else {
                                    None
                                }
                            });
                            
                            if let Some(found) = cached {
                                return Value::Object(found);
                            }

                            for ind in individuals {
                                let env_b = ind.env.read().unwrap();
                                let store = &env_b.store;
                                let ind_x = store.get("x").map_or(0, |v| v.to_int());
                                let ind_y = store.get("y").map_or(0, |v| v.to_int());
                                if ind_x == x && ind_y == y {
                                    return Value::Object(ind.env.clone());
                                }
                            }
                        }
                        Value::Int(0)
                    }

            //dist(obj1, obj2) - distance between two objects
            "dist" => {
                if args.len() >= 2 {
                    let obj1 = args[0].eval_to_val(env.clone(), individuals);
                    let obj2 = args[1].eval_to_val(env, individuals);
                    
                    if let (Value::Object(o1), Value::Object(o2)) = (obj1, obj2) {
                    let x1 = o1.read().unwrap().store.get("x").map_or(0, |v| v.to_int());
                    let y1 = o1.read().unwrap().store.get("y").map_or(0, |v| v.to_int());
                    let x2 = o2.read().unwrap().store.get("x").map_or(0, |v| v.to_int());
                    let y2 = o2.read().unwrap().store.get("y").map_or(0, |v| v.to_int());
                        
                        let dx = (x1 - x2) as f64;
                        let dy = (y1 - y2) as f64;
                        let distance = (dx * dx + dy * dy).sqrt();
                        
                        return Value::Int(distance as i32);
                    }
                }
                Value::Int(0)
            }
            
            //draw_rect(x, y, w, h, r, g, b)
            "draw_rect" => {
                if args.len() >= 4 {
                    let x = args[0].eval(env.clone(), individuals) as f32;
                    let y = args[1].eval(env.clone(), individuals) as f32;
                    let w = args[2].eval(env.clone(), individuals) as f32;
                    let h = args[3].eval(env.clone(), individuals) as f32;
                    
                    //colors are optional, default to white
                    let r = if args.len() > 4 { args[4].eval(env.clone(), individuals) as u8 } else { 255 };
                    let g = if args.len() > 5 { args[5].eval(env.clone(), individuals) as u8 } else { 255 };
                    let b = if args.len() > 6 { args[6].eval(env, individuals) as u8 } else { 255 };
                    
                    DRAW_COMMANDS.with(|cmds| {
                        cmds.borrow_mut().push(DrawCmd::Rect { x, y, w, h, r, g, b });
                    });
                }
                Value::Int(0)
            }
            
            //draw_line(x1, y1, x2, y2, r, g, b, thickness)
            "draw_line" => {
                if args.len() >= 4 {
                    let x1 = args[0].eval(env.clone(), individuals) as f32;
                    let y1 = args[1].eval(env.clone(), individuals) as f32;
                    let x2 = args[2].eval(env.clone(), individuals) as f32;
                    let y2 = args[3].eval(env.clone(), individuals) as f32;
                    
                    let r = if args.len() > 4 { args[4].eval(env.clone(), individuals) as u8 } else { 255 };
                    let g = if args.len() > 5 { args[5].eval(env.clone(), individuals) as u8 } else { 255 };
                    let b = if args.len() > 6 { args[6].eval(env.clone(), individuals) as u8 } else { 255 };
                    let thickness = if args.len() > 7 { args[7].eval(env, individuals) as f32 } else { 1.0 };
                    
                    DRAW_COMMANDS.with(|cmds| {
                        cmds.borrow_mut().push(DrawCmd::Line { x1, y1, x2, y2, r, g, b, thickness });
                    });
                }
                Value::Int(0)
            }
            
            //draw_circle(x, y, radius, r, g, b)
            "draw_circle" => {
                if args.len() >= 3 {
                    let x = args[0].eval(env.clone(), individuals) as f32;
                    let y = args[1].eval(env.clone(), individuals) as f32;
                    let radius = args[2].eval(env.clone(), individuals) as f32;
                    
                    let r = if args.len() > 3 { args[3].eval(env.clone(), individuals) as u8 } else { 255 };
                    let g = if args.len() > 4 { args[4].eval(env.clone(), individuals) as u8 } else { 255 };
                    let b = if args.len() > 5 { args[5].eval(env, individuals) as u8 } else { 255 };
                    
                    DRAW_COMMANDS.with(|cmds| {
                        cmds.borrow_mut().push(DrawCmd::Circle { x, y, radius, r, g, b });
                    });
                }
                Value::Int(0)
            }
            
            //for unknown functions, just return 0
            _ => Value::Int(self.eval(env, individuals)),
        }
    }
}

//boolean expression evaluation

impl BExp {
    //check if a condition is true
    pub fn eval(&self, env: Arc<RwLock<Environment>>, individuals: &[Individual]) -> bool {
        match self {
            //a && b
            BExp::And(left, right) => {
                left.eval(env.clone(), individuals) && right.eval(env, individuals)
            }
            
            //a || b
            BExp::Or(left, right) => {
                left.eval(env.clone(), individuals) || right.eval(env, individuals)
            }
            
            //a == b (works for strings too!)
            BExp::Equal(left, right) => {
                let left_val = left.eval_to_val(env.clone(), individuals);
                let right_val = right.eval_to_val(env, individuals);
                values_are_equal(&left_val, &right_val)
            }
            
            //a != b
            BExp::NotEqual(left, right) => {
                let left_val = left.eval_to_val(env.clone(), individuals);
                let right_val = right.eval_to_val(env, individuals);
                !values_are_equal(&left_val, &right_val)
            }
            
            //a > b
            BExp::Greater(left, right) => {
                left.eval(env.clone(), individuals) > right.eval(env, individuals)
            }
            
            //a < b
            BExp::Less(left, right) => {
                left.eval(env.clone(), individuals) < right.eval(env, individuals)
            }
            
            //a >= b
            BExp::GreaterEqual(left, right) => {
                left.eval(env.clone(), individuals) >= right.eval(env, individuals)
            }
            
            //a <= b
            BExp::LessEqual(left, right) => {
                left.eval(env.clone(), individuals) <= right.eval(env, individuals)
            }
        }
    }
}

//helper function to compare two values
fn values_are_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::String(x), Value::String(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Object(x), Value::Object(y)) => Arc::ptr_eq(x, y),
        //null checks (0 means "nothing")
        (Value::Int(0), Value::Object(_)) => false,
        (Value::Object(_), Value::Int(0)) => false,
        _ => false,
    }
}

//command execution

impl Command {
    //run a command and maybe return a value (for return statements)
    pub fn execute(
        &self,
        env: Arc<RwLock<Environment>>,
        individuals: &[Individual],
        spawner: &mut Vec<Individual>,
        program: &Program,
    ) -> Option<Value> {
        match self {
            //just evaluate an expression (for function calls like push())
            Command::Exp(exp, _line) => {
                exp.eval_to_val(env, individuals);
                None
            }
            
            //spawn species @ (x, y)
            Command::Spawn { species, x, y, line: _line } => {
                if let Some(species_def) = program.species_block.get(species) {
                    let x_pos = x.eval(env.clone(), individuals);
                    let y_pos = y.eval(env, individuals);
                    
                    //create new individual
                    let new_env = Environment::new();
                    {
                        let mut env_mut = new_env.write().unwrap();
                        env_mut.store.insert("species".to_string(), Value::String(species.clone()));
                        
                        //copy default properties from species definition
                        for (prop_name, prop_exp) in &species_def.properties {
                            let value = prop_exp.eval_to_val(new_env.clone(), individuals);
                            env_mut.store.insert(prop_name.clone(), value);
                        }
                        
                        //set position
                        env_mut.store.insert("x".to_string(), Value::Int(x_pos));
                        env_mut.store.insert("y".to_string(), Value::Int(y_pos));
                    }
                    
                    spawner.push(Individual {
                        species: species.clone(),
                        env: new_env,
                    });
                }
                None
            }
            
            //print(a, b, c)
            Command::Print(expressions, _line) => {
                let mut parts = Vec::new();
                for exp in expressions {
                    let value = exp.eval_to_val(env.clone(), individuals);
                    parts.push(value.to_string());
                }
                println!("{}", parts.join(" "));
                None
            }
            
            //x = value
            Command::Assign { target, value, line: _line } => {
                let new_value = value.eval_to_val(env.clone(), individuals);
                
                match target {
                    //simple variable: x = 5
                    Exp::Var(name, _l) => {
                        let mut env_ref = env.write().unwrap();
                        env_ref.store.insert(name.clone(), new_value);
                    }
                    //object field: self.x = 5
                    Exp::Dot(obj_exp, field, _l) => {
                        if let Value::Object(obj_env) = obj_exp.eval_to_val(env, individuals) {
                            obj_env.write().unwrap().store.insert(field.clone(), new_value);
                        }
                    }
                    //list index: genes[i] = 5
                    Exp::Index(list_exp, idx_exp, _l) => {
                        if let Value::List(list) = list_exp.eval_to_val(env.clone(), individuals) {
                            let idx = idx_exp.eval(env, individuals) as usize;
                            let mut borrowed = list.write().unwrap();
                            if idx < borrowed.len() {
                                borrowed[idx] = new_value;
                            }
                        }
                    }
                    _ => {}
                }
                None
            }
            
            //if (condition) { ... } else { .... }
            Command::If { condition, then_block, else_block, line: _line } => {
                if condition.eval(env.clone(), individuals) {
                    //run then block
                    for cmd in then_block {
                        let result = cmd.execute(env.clone(), individuals, spawner, program);
                        if result.is_some() {
                            return result; 
                        }
                    }
                } else if let Some(else_cmds) = else_block {
                    //run else block
                    for cmd in else_cmds {
                        let result = cmd.execute(env.clone(), individuals, spawner, program);
                        if result.is_some() {
                            return result;
                        }
                    }
                }
                None
            }
            
            //while (cond) { ... }
            Command::While { condition, body, line: _line } => {
                while condition.eval(env.clone(), individuals) {
                    for cmd in body {
                        let result = cmd.execute(env.clone(), individuals, spawner, program);
                        if result.is_some() {
                            return result;
                        }
                    }
                }
                None
            }
            
            //for item in environment { ... }
            //only for environment any other for just use while instead </3
            Command::For { var, collection, body, line: _line } => {
                if collection == "environment" {
                    for ind in individuals {
                        env.write().unwrap().store.insert(var.clone(), Value::Object(ind.env.clone()));
                        for cmd in body {
                            let result = cmd.execute(env.clone(), individuals, spawner, program);
                            if result.is_some() {
                                return result;
                            }
                        }
                    }
                }
                None
            }
            
            //return value
            Command::Return(exp, _line) => {
                let value = exp.eval_to_val(env, individuals);
                Some(value)
            }
        }
    }
}
