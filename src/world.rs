//world.rs -> world simulation logic ->
// running steps calculating fitness (and managing evolution -- no more, moved to evolution.rs)

use std::collections::HashMap;

use crate::types::*;
use crate::eval::{GRID_CACHE, WORLD_DIMENSIONS};

impl World {
    //run spawn block to create initial individuals
    pub fn spawn(&mut self) {
        let mut spawner = Vec::new();
        let env = Environment::new();
        
        for cmd in &self.program.spawns_block {
            cmd.execute(env.clone(), &self.individuals, &mut spawner, &self.program);
        }
        
        self.individuals.extend(spawner);
    }

    //run one simulation step for all individuals
    pub fn step(&mut self) {
        if self.record_history {
            let mut step_snapshot = Vec::new();
            for ind in &self.individuals {
                let cloned_ind = ind.deep_clone();
                step_snapshot.push(cloned_ind);
            }
            self.history.push(step_snapshot);
        }

        //set up world dimensions
        WORLD_DIMENSIONS.with(|d| *d.borrow_mut() = (self.width, self.height));
        
        //build position cache
        self.build_grid_cache();

        let mut spawner = Vec::new();
        
        //optimization: use a shared snapshot for read-only environment access if needed
        //but for routine execution just iterate
        for i in 0..self.individuals.len() {
            //get the species definition
            let species_name = self.individuals[i].species.clone();
            if let Some(species_def) = self.program.species_block.get(&species_name) {
                //get the routine to execute
                if let Some(routine) = self.program.routines_block.get(&species_def.routine_call) {
                    let env = self.individuals[i].env.clone();
                    env.write().unwrap().store.insert("self".to_string(), Value::Object(env.clone()));
                    
                    for cmd in &routine.body {
                        //pass individuals slice directly instead of cloning
                        cmd.execute(env.clone(), &self.individuals, &mut spawner, &self.program);
                    }
                }
            }
        }
        
        self.individuals.extend(spawner);
        self.clear_grid_cache();
    }

    //calculate fitness for a single individual
    pub fn calculate_fitness(&self, ind: &Individual) -> i32 {
        let fitness_def = &self.program.fitness_block;
        
        if !fitness_def.commands.is_empty() {
            //use the individual's own environment directly
            //this means all variables created during fitness go into the individual
            let env = ind.env.clone();
            env.write().unwrap().store.insert("self".to_string(), Value::Object(ind.env.clone()));
            let mut spawner = Vec::new();
            
            for cmd in &fitness_def.commands {
                let result = cmd.execute(env.clone(), &self.individuals, &mut spawner, &self.program);
                if let Some(val) = result {
                    return val.to_int();
                }
            }
            
            //if no return statement, check if 'score' variable was set
            let store = &env.read().unwrap().store;
            let score = store.get("score").map_or(0, |v| v.to_int());
            if score != 0 {
                return score;
            }
        }
        0
    }

    //calculate fitness for all individuals and return best score
    pub fn calculate_total_fitness(&mut self) -> i32 {
        WORLD_DIMENSIONS.with(|d| *d.borrow_mut() = (self.width, self.height));
        self.build_grid_cache();

        let mut best = 0;
        
        for i in 0..self.individuals.len() {
            let score = self.calculate_fitness(&self.individuals[i]);
            self.individuals[i].env.write().unwrap().store.insert("fitness".to_string(), Value::Int(score));
            if score > best {
                best = score;
            }
        }
        
        self.fitness = best;
        self.clear_grid_cache();
        best
    }

    //apply mutations to all individuals
    pub fn mutate(&mut self) {
        if self.individuals.is_empty() {
            return;
        }

        //mutate everyone no selection
        let individuals_snapshot = self.individuals.clone();
        for offspring in &mut self.individuals {
            //apply mutation rule
            if let Some(rule) = self.program.mutations_block.iter()
                .find(|r| r.action == "mutation") 
            {
                if rand::random::<f32>() < rule.probability {
                    if let Some(body) = &rule.body {
                        let env = offspring.env.clone();
                        env.write().unwrap().store.insert("self".to_string(), Value::Object(offspring.env.clone()));
                        
                        let mut spawner = Vec::new();
                        for cmd in body {
                            cmd.execute(env.clone(), &individuals_snapshot, &mut spawner, &self.program);
                        }
                    }
                }
            }
        }
    }

    //helper methods

    //build grid cache for fast position lookups
    fn build_grid_cache(&self) {
        if self.individuals.len() <= 1 {
            return;
        }
        
        let mut grid_map = HashMap::new();
        for ind in &self.individuals {
            let env_b = ind.env.read().unwrap();
            let store = &env_b.store;
            if let Some(Value::Int(x)) = store.get("x") {
                if let Some(Value::Int(y)) = store.get("y") {
                    grid_map.insert((*x, *y), ind.env.clone());
                }
            }
        }
        GRID_CACHE.with(|cache| *cache.borrow_mut() = Some(grid_map));
    }

    //clear the grid cache
    fn clear_grid_cache(&self) {
        GRID_CACHE.with(|cache| *cache.borrow_mut() = None);
    }
}
