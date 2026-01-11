//gui.rs - graphical user interface using egui

use std::collections::HashMap;
use std::sync::Arc;
use rayon::prelude::*;

use eframe::egui;

use crate::types::*;
use crate::eval::{DRAW_COMMANDS, GRID_CACHE, WORLD_DIMENSIONS};
use crate::evolution::{
    snapshot_individuals, create_next_generation, 
    clear_snapshot_memory, clear_world_history
};

//application state

pub struct SimApp {
    pub instances: Vec<World>,
    pub history: Vec<GenerationSnapshot>,
    pub current_gen_idx: usize,
    pub current_step_idx: i32,
    pub world_width: i32,
    pub world_height: i32,
    pub program: Arc<Program>,
    pub num_generations: i32,
    pub num_instances: i32,
    pub current_gen: i32,
    pub running: bool,
    pub global_best_fitness: i32,
}

impl SimApp {
    pub fn new(
        instances: Vec<World>,
        program: Arc<Program>,
        num_generations: i32,
        num_instances: i32,
    ) -> Self {
        Self {
            instances,
            history: Vec::new(),
            current_gen_idx: 0,
            current_step_idx: 1,
            world_width: program.env_width,
            world_height: program.env_height,
            program,
            num_generations,
            num_instances,
            current_gen: 0,
            running: false,
            global_best_fitness: 0,
        }
    }

    //run one generation of evolution
    pub fn run_generation(&mut self) {
        let start = std::time::Instant::now();
        
        if self.current_gen >= self.num_generations {
            self.running = false;
            return;
        }
        
        self.current_gen += 1;
        let g = self.current_gen;

        //run simulation steps in parallel using Rayon
        let env_steps = self.program.env_steps;
        
        //enable history recording
        for w in &mut self.instances {
            w.record_history = true;
            w.history.clear();
        }

        self.instances.par_iter_mut().for_each(|world| {
            world.generation = g;
            for _ in 0..env_steps {
                world.step();
            }
            //capture final state as a snapshot
            if world.record_history {
                let mut final_snapshot = Vec::new();
                for ind in &world.individuals {
                    let cloned_ind = ind.deep_clone();
                    final_snapshot.push(cloned_ind);
                }
                world.history.push(final_snapshot);
            }
            world.calculate_total_fitness();
        });

        //sort by fitness
        let mut indices: Vec<usize> = Vec::new();
        for i in 0..self.instances.len() {
            indices.push(i);
        }
        indices.sort_by_key(|&i| -self.instances[i].fitness);
        
        let mut sorted_instances = Vec::new();
        for &i in &indices {
            sorted_instances.push(self.instances[i].take());
        }
        self.instances = sorted_instances;
        
        //extract history from the best instance
        let raw_history = std::mem::take(&mut self.instances[0].history);
        let mut best_history = Vec::new();
        for step_individuals in raw_history {
            let step_snapshot = snapshot_individuals(&step_individuals, &self.program);
            //break reference cycles in the original heavy data
            for ind in step_individuals {
                ind.env.write().unwrap().store.clear();
            }
            best_history.push(step_snapshot);
        }
        
        //clear history from other instances
        clear_world_history(&mut self.instances);

        // record statistics
        let mut total_fitness: i32 = 0;
        for w in &self.instances {
            total_fitness += w.fitness;
        }
        let avg = total_fitness / self.num_instances.max(1);
        let best = self.instances[0].fitness;
        if best > self.global_best_fitness {
            self.global_best_fitness = best;
        }

        //store snapshot for visualization
        let snapshot = GenerationSnapshot {
            avg_fitness: avg,
            best_fitness: best,
            individuals: snapshot_individuals(&self.instances[0].individuals, &self.program),
            step_history: best_history,
        };
        
        //track if it was at the end before adding new history
        let was_at_end = self.current_gen_idx >= self.history.len().saturating_sub(1);
        
        self.history.push(snapshot);
        
        //limit history to prevent memory leak
        // rip my laptop learned from experience </3
        if self.history.len() > 100 {
            let removed = self.history.remove(0);
            clear_snapshot_memory(&removed);
            if self.current_gen_idx > 0 {
                self.current_gen_idx -= 1;
            }
        }
        
        //auto-follow-> if viewing latest, move to new latest
        if was_at_end {
            self.current_gen_idx = self.history.len().saturating_sub(1);
            self.current_step_idx = 0;
        }

        let duration = start.elapsed();
        println!("[Gen {}] Avg: {}, Best: {} (took {:?})", g, avg, best, duration);

        //create next generation
        self.instances = create_next_generation(
            &mut self.instances,
            &self.program,
            self.num_instances,
            self.current_gen,
        );
    }

    //reset to initial state
    pub fn reset(&mut self) {
        self.current_gen = 0;
        self.history.clear();
        self.current_gen_idx = 0;
        self.current_step_idx = 1;
        self.running = false;
        self.global_best_fitness = 0;
        
        let mut new_instances = Vec::new();
        for i in 0..self.num_instances {
            let mut w = World::new(self.program.clone(), i);
            w.spawn();
            new_instances.push(w);
        }
        self.instances = new_instances;
    }
}

//egui application - ui rendering

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
            
            ui.heading("Simulanka Evolution Simulator");

            //control buttons - easier to make than i thought
            ui.horizontal(|ui| {
                if ui.button(if self.running { "Stop" } else { "Start" }).clicked() {
                    if self.validate_can_run() {
                        self.running = !self.running;
                    }
                }
                if ui.button("Next").clicked() {
                    self.run_generation();
                }
                if ui.button("Reset").clicked() {
                    self.reset();
                }
                if ui.button("<- Prev").clicked() && self.current_gen_idx > 0 {
                    self.current_gen_idx -= 1;
                }
                ui.label(format!("Gen {} / {}", self.current_gen_idx + 1, self.history.len()));
                if ui.button("Next ->").clicked() && self.current_gen_idx < self.history.len().saturating_sub(1) {
                    self.current_gen_idx += 1;
                }
            });

            //status display
            ui.horizontal(|ui| {
                ui.label("Status:");
                if self.num_instances == 0 {
                    ui.colored_label(egui::Color32::RED, "Error: No instances");
                } else if self.num_generations == 0 {
                    ui.colored_label(egui::Color32::YELLOW, "Warning: No generations");
                } else if self.current_gen >= self.num_generations {
                    ui.colored_label(egui::Color32::GREEN, "Finished");
                } else if self.running {
                    ui.colored_label(egui::Color32::LIGHT_BLUE, "Running (Auto-following)...");
                } else {
                    ui.label("Ready");
                }
            });

            if self.history.is_empty() {
                ui.label("Press Start or Step to begin evolution.");
                return;
            }

            let snapshot = &self.history[self.current_gen_idx];
            ui.label(format!(
                "Avg: {}, Gen Best: {}, Global Best: {}",
                snapshot.avg_fitness, snapshot.best_fitness, self.global_best_fitness
            ));

            //generation slider
            if ui.add(egui::Slider::new(&mut self.current_gen_idx, 0..=self.history.len().saturating_sub(1))
                .text("View Gen")).changed() 
            {
                self.current_step_idx = 0;
            }

            //step slider
            if !snapshot.step_history.is_empty() {
                let max_steps = snapshot.step_history.len().saturating_sub(1) as i32;
                ui.add(egui::Slider::new(&mut self.current_step_idx, 0..=max_steps).text("Step"));
            }

            ui.separator();

            //render visualization
            self.render_visualization(ui, snapshot);
        });
    }
}

//visualization rendering

impl SimApp {
    fn validate_can_run(&self) -> bool {
        if self.num_instances == 0 {
            println!("Error: No instances defined in EVOLVE block.");
            return false;
        }
        if self.num_generations == 0 {
            println!("Error: No generations defined in EVOLVE block.");
            return false;
        }
        true
    }

    fn render_visualization(&self, ui: &mut egui::Ui, snapshot: &GenerationSnapshot) {
        //clear previous draw commands
        DRAW_COMMANDS.with(|cmds| cmds.borrow_mut().clear());

        //execute VISUALIZE block
        if !self.program.visualize_block.is_empty() {
            self.execute_visualize_block(snapshot);
        }

        //draw canvas
        let size = 600.0;
        let (rect, _) = ui.allocate_at_least(egui::vec2(size, size), egui::Sense::hover());
        let painter = ui.painter();
        
        painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(20, 20, 20));
        
        //draw all commands
        DRAW_COMMANDS.with(|cmds| {
            for cmd in cmds.borrow().iter() {
                match cmd {
                    DrawCmd::Rect { x, y, w, h, r, g, b } => {
                        let min = rect.min + egui::vec2(*x, *y);
                        painter.rect_filled(
                            egui::Rect::from_min_size(min, egui::vec2(*w, *h)),
                            0.0,
                            egui::Color32::from_rgb(*r, *g, *b),
                        );
                    }
                    DrawCmd::Line { x1, y1, x2, y2, r, g, b, thickness } => {
                        painter.line_segment(
                            [rect.min + egui::vec2(*x1, *y1), rect.min + egui::vec2(*x2, *y2)],
                            egui::Stroke::new(*thickness, egui::Color32::from_rgb(*r, *g, *b)),
                        );
                    }
                    DrawCmd::Circle { x, y, radius, r, g, b } => {
                        painter.circle_filled(
                            rect.min + egui::vec2(*x, *y),
                            *radius,
                            egui::Color32::from_rgb(*r, *g, *b),
                        );
                    }
                }
            }
        });
    }

    fn execute_visualize_block(&self, snapshot: &GenerationSnapshot) {
        WORLD_DIMENSIONS.with(|d| *d.borrow_mut() = (self.world_width, self.world_height));
        
        let viz_env = Environment::new();
        {
            let mut env_mut = viz_env.write().unwrap();
            env_mut.store.insert("width".to_string(), Value::Int(self.world_width));
            env_mut.store.insert("height".to_string(), Value::Int(self.world_height));
        }
        
        //use the snapshot directly
        let viz_individuals = if !snapshot.step_history.is_empty() {
            let idx = (self.current_step_idx as usize).min(snapshot.step_history.len().saturating_sub(1));
            &snapshot.step_history[idx]
        } else {
            &snapshot.individuals
        };

        // set up grid cache for visualization
        let mut grid_map = HashMap::new();
        for ind in viz_individuals {
            let env_b = ind.env.read().unwrap();
            let store = &env_b.store;
            if let Some(Value::Int(x)) = store.get("x") {
                if let Some(Value::Int(y)) = store.get("y") {
                    grid_map.insert((*x, *y), ind.env.clone());
                }
            }
        }
        GRID_CACHE.with(|cache| *cache.borrow_mut() = Some(grid_map));

        //execute visualize commands
        let mut spawner = Vec::new();
        for cmd in &self.program.visualize_block {
            cmd.execute(viz_env.clone(), viz_individuals, &mut spawner, &self.program);
        }

        GRID_CACHE.with(|cache| *cache.borrow_mut() = None);
    }
}
