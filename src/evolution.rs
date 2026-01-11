//evolution.rs - evolutionary algorithm logic
//handles the core evolutionary operations:
//- generation creation and selection
//- crossover between parents
//- memory management for generations
//- snapshot creation for history

use std::collections::HashMap;
use std::sync::Arc;

use crate::types::*;

//create a snapshot of individuals for history
pub fn snapshot_individuals(individuals: &[Individual], program: &Program) -> Vec<Individual> {
    let mut snapshot = Vec::new();
    for ind in individuals {
        let new_env = Environment::new();
        
        //optimization- only copy persistent state defined in species scheme.
        //this acts as a garbage collector temporary variables created are not stored in the history, preventing memory bloat.
        let mut store = HashMap::new();
        let env_read = ind.env.read().unwrap();

        //1. copy 'x', 'y' (system variables)
        if let Some(val) = env_read.store.get("x") { store.insert("x".to_string(), val.clone()); }
        if let Some(val) = env_read.store.get("y") { store.insert("y".to_string(), val.clone()); }
        
        //2. copy species string (needed for species checking in visualize/fitness)
        store.insert("species".to_string(), Value::String(ind.species.clone()));

        //3. copy variables defined in the species block (genetic/state memory)
        if let Some(species_def) = program.species_block.get(&ind.species) {
            for key in species_def.properties.keys() {
                if let Some(val) = env_read.store.get(key) {
                    store.insert(key.clone(), val.deep_copy());
                }
            }
        } else {
            //fallback for unknown species: copy everything (safety net)
            store = env_read.deep_copy_store();
        }
        
        //fix self pointer in snapshot so it doesn't point to original
        store.insert("self".to_string(), Value::Object(new_env.clone()));
        
        new_env.write().unwrap().store = store;
        snapshot.push(Individual {
            species: ind.species.clone(),
            env: new_env,
        });
    }
    snapshot
}

//create next generation from current best instances
pub fn create_next_generation(
    instances: &mut Vec<World>,
    program: &Arc<Program>,
    num_instances: i32,
    current_gen: i32,
) -> Vec<World> {
    let keep_count = (num_instances / 2).max(1) as usize;
    let mut next_gen = Vec::new();
    
    for i in 0..num_instances as usize {
        let parent_idx = i % keep_count;
        let mut child = World::new(program.clone(), i as i32);
        child.generation = current_gen;
        
        //copy individuals from parent
        for ind in &instances[parent_idx].individuals {
            let child_env = Environment::new();
            
            //optimization: garbage collect transient variables.
            // recreate the child based only on the species schema (dna) plus its position. any temporary variables are dropped.
            let mut store = HashMap::new();
            let parent_env_read = ind.env.read().unwrap();
            
            //1. copy position
            if let Some(val) = parent_env_read.store.get("x") { store.insert("x".to_string(), val.clone()); }
            if let Some(val) = parent_env_read.store.get("y") { store.insert("y".to_string(), val.clone()); }
            
            //2. copy species string (needed for species checking in fitness/routines)
            store.insert("species".to_string(), Value::String(ind.species.clone()));

            //3. copy schema properties (deep copy)
            if let Some(species_def) = program.species_block.get(&ind.species) {
                for key in species_def.properties.keys() {
                    if let Some(val) = parent_env_read.store.get(key) {
                        store.insert(key.clone(), val.deep_copy());
                    }
                }
            } else {
                store = parent_env_read.deep_copy_store();
            }
            
            //fix self to point to new environment
            store.insert("self".to_string(), Value::Object(child_env.clone()));
            
            child_env.write().unwrap().store = store;
            child.individuals.push(Individual {
                species: ind.species.clone(),
                env: child_env,
            });
        }
        
        //apply crossover for non-elite children
        if i >= keep_count {
            apply_crossover(&mut child, instances, i, keep_count, program);
        }

        child.mutate();
        next_gen.push(child);
    }
    
    //IMPORTANT: break potential reference cycles in the old generation
    //because we use arc cycles (like agents pointing to each other) will never be freed otherwise.
    //(aka my hungry ass got memory leaks in rust :sob: :pray:)

    clear_generation_memory(instances);

    next_gen
}

//apply crossover between parents
fn apply_crossover(
    child: &mut World,
    instances: &[World],
    i: usize,
    keep_count: usize,
    program: &Program,
) {
    let p2_idx = (i + 1) % keep_count;
    let p2 = &instances[p2_idx];
    
    if let Some(rule) = program.mutations_block.iter()
        .find(|r| r.action == "crossover") 
    {
        if let Some(body) = &rule.body {
            for j in 0..child.individuals.len() {
                let crossover_env = Environment::new();
                {
                    let mut env_mut = crossover_env.write().unwrap();
                    env_mut.store.insert("parent1".to_string(), Value::Object(child.individuals[j].env.clone()));
                    env_mut.store.insert("parent2".to_string(), Value::Object(p2.individuals[j].env.clone()));
                    env_mut.store.insert("child".to_string(), Value::Object(child.individuals[j].env.clone()));
                }
                
                let mut spawner = Vec::new();
                for cmd in body {
                    cmd.execute(crossover_env.clone(), &[], &mut spawner, program);
                }
                
                //memory fix: clear crossover_env to break reference cycles
                crossover_env.write().unwrap().store.clear();
            }
        }
    }
}

//clear memory from a generation to prevent memory leaks
pub fn clear_generation_memory(instances: &mut [World]) {
    for world in instances {
        for ind in &mut world.individuals {
            ind.env.write().unwrap().store.clear();
        }
    }
}

//clear memory from a snapshot
pub fn clear_snapshot_memory(snapshot: &GenerationSnapshot) {
    for ind in &snapshot.individuals {
        ind.env.write().unwrap().store.clear();
    }
    for step in &snapshot.step_history {
        for ind in step {
            ind.env.write().unwrap().store.clear();
        }
    }
}

//clear history from world instances
pub fn clear_world_history(instances: &mut [World]) {
    for w in instances {
        for step in &w.history {
            for ind in step {
                ind.env.write().unwrap().store.clear();
            }
        }
        w.history.clear();
        w.record_history = false;
    }
}
