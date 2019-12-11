extern crate array2d;
#[macro_use] extern crate clap;
extern crate fnv;
extern crate indicatif;
extern crate rayon;

mod bit_mat;
mod constraints;
mod edge_plan;
mod journey_plan;
mod parcel_plan;
mod read;
mod write;

use array2d::{Array2D};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::{fs, io, sync::{Arc}, thread, time};
use crate::{
  edge_plan::{init_edge_state, plan_edges, plan_edges_hub, plan_edges_all},
  journey_plan::{JourneyProblem, plan_journeys},
  parcel_plan::{ParcelProblem, Action, plan_parcels},
};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug)]
struct VehicleConfig {
  cap: u32,
  transfer_cost: u64,
  go_cost: u64,
}

#[derive(Debug)]
struct Config {
  truck: VehicleConfig,
  airplane: VehicleConfig,
}

#[derive(Debug)]
pub struct Problem {
  pub depos: Vec<(u32, u32)>,
  pub cities: Vec<CityProblem>,
  pub air: AirProblem,
  pub parcel_count: u32,
}

#[derive(Debug)]
pub struct Plan {
  pub truck_actions_1: Vec<Action>,
  pub airplane_actions_2: Vec<Action>,
  pub truck_actions_3: Vec<Action>,
  pub cost: u64,
  pub min_cost: u64,
}

#[derive(Debug)]
pub struct CityProblem {
  pub depo_ids: Vec<u32>,
  pub airport_depo: u32,
  pub truck_ids: Vec<u32>,
  pub truck_depos: Vec<u32>,
  pub inner_parcel_ids: Array2D<Vec<u32>>,
  pub outbound_parcel_ids: Vec<Vec<u32>>,
  pub inbound_parcel_ids: Vec<Vec<u32>>,
  pub parcel_count: u32,
}

#[derive(Debug)]
struct CityPlan {
  before_air_actions: Vec<Action>,
  after_air_actions: Vec<Action>,
  cost: u64,
  min_cost: u64,
}

#[derive(Debug)]
pub struct AirProblem {
  pub airport_ids: Vec<u32>,
  pub airplane_airports: Vec<u32>,
  pub parcel_ids: Array2D<Vec<u32>>,
}

#[derive(Debug)]
struct AirPlan {
  air_actions: Vec<Action>,
  cost: u64,
  min_cost: u64,
}


fn main() -> Result<()> {
  let start_instant = time::Instant::now();

  // read command-line arguments
  let args = clap_app!(Logistics =>
    (version: crate_version!())
    (author: crate_authors!())
    (about: crate_description!())
    (@arg INPUT: +required "Input file with problem defition (use - for stdin)")
    (@arg OUTPUT: +required "Output file with problem solution (use - for stdout)")
  ).get_matches();
  let input_path = args.value_of_os("INPUT").unwrap();
  let output_path = args.value_of_os("OUTPUT").unwrap();

  let cfg = Config {
    truck: VehicleConfig { cap: 4, transfer_cost: 2+2, go_cost: 17 },
    airplane: VehicleConfig { cap: 30, transfer_cost: 14+11, go_cost: 1000 },
  };

  // read problem from file or stdin
  let problem = {
    let bar = ProgressBar::new_spinner().with_style(spinner_style());
    bar.set_prefix("Input");
    let problem = if input_path != "-" {
      let file = fs::File::open(input_path)?;
      let mut input = io::BufReader::new(file);
      read::read_problem(&mut input, &bar)?
    } else {
      let stdin = io::stdin();
      let mut input = stdin.lock();
      read::read_problem(&mut input, &bar)?
    };
    bar.finish_and_clear();
    problem
  };

  let parcel_count = problem.parcel_count;
  eprintln!("Problem has {} cities, {} depos, {} parcels",
    problem.cities.len(), problem.depos.len(), problem.parcel_count);

  // solve the problem
  let plan = {
    let plan = solve_problem(problem, &cfg);

    eprintln!("Plan cost {}, min cost {} (gap <= {:.3}), avg {:.2} per parcel",
      plan.cost, plan.min_cost,
      plan.cost as f64 / plan.min_cost as f64 - 1.0,
      plan.cost as f64 / parcel_count as f64);
    plan
  };

  // write the plan to file or stdout
  {
    let bar = ProgressBar::new_spinner().with_style(spinner_style());
    bar.set_prefix("Output");
    if output_path != "-" {
      let file = fs::File::create(output_path)?;
      let mut output = io::BufWriter::new(file);
      write::write_plan(&mut output, &plan, &bar)?;
    } else {
      let stdout = io::stdout();
      let mut output = io::BufWriter::new(stdout.lock());
      write::write_plan(&mut output, &plan, &bar)?;
    }
    bar.finish_and_clear();
  };

  let duration = time::Instant::now().duration_since(start_instant);
  eprintln!("Finished in {:.2} s", duration.as_secs_f64());

  Ok(())
}

fn solve_problem(problem: Problem, cfg: &Config) -> Plan {
  let air_problem = problem.air;
  let city_problems = problem.cities;

  // prepare the parallel progress bars
  let progress = Arc::new(MultiProgress::new());
  let air_bar = progress.add(ProgressBar::new(0).with_style(bar_style()));
  let cities_bar = progress.add(
    ProgressBar::new(city_problems.len() as u64).with_style(bar_style()));

  let progress_clone = progress.clone();
  let progress_thread = thread::spawn(move || progress_clone.join_and_clear().unwrap());

  // solve the problems in parallel, handling the progress bars
  let (air_plan, city_plans) = rayon::join(
    move || {
      air_bar.set_prefix("Airplanes");
      let plan = solve_air_problem(air_problem, cfg, &air_bar);
      air_bar.finish_and_clear();
      plan
    },
    move || {
      cities_bar.set_prefix("Cities   ");
      let plans = city_problems.into_iter().enumerate()
        .par_bridge()
        .map(|(i, city_problem)| {
          let city_bar =
            if city_problem.parcel_count >= 1000 {
              progress.add(ProgressBar::new(0).with_style(bar_style()))
            } else {
              ProgressBar::hidden()
            };

          city_bar.set_prefix(&format!("City {}", i));
          let plan = solve_city_problem(city_problem, cfg, &city_bar);
          city_bar.finish_and_clear();
          cities_bar.inc(1);
          plan
        })
        .collect::<Vec<_>>();
      cities_bar.finish_and_clear();
      plans
    });
  progress_thread.join().unwrap();

  let cost = city_plans.iter().map(|p| p.cost).sum::<u64>() + air_plan.cost;
  let min_cost = city_plans.iter().map(|p| p.min_cost).sum::<u64>() + air_plan.min_cost;

  let mut truck_actions_1 = Vec::new();
  let mut truck_actions_3 = Vec::new();
  for city_plan in city_plans {
    truck_actions_1.extend(city_plan.before_air_actions);
    truck_actions_3.extend(city_plan.after_air_actions);
  }

  let airplane_actions_2 = air_plan.air_actions;
  Plan { truck_actions_1, airplane_actions_2, truck_actions_3, cost, min_cost }
}

fn solve_city_problem(problem: CityProblem, cfg: &Config, bar: &ProgressBar) -> CityPlan {
  bar.set_message("initializing");

  let depo_count = problem.depo_ids.len();
  let airport = problem.airport_depo as usize;

  // add the parcels going to and from the airport to parcel_ids
  let mut parcel_ids = problem.inner_parcel_ids;
  for depo in 0..depo_count {
    parcel_ids[(depo, airport)].extend(&problem.outbound_parcel_ids[depo]);
    parcel_ids[(airport, depo)].extend(&problem.inbound_parcel_ids[depo]);
  }

  // calculate the p_mat
  let p_mat = Array2D::from_iter_row_major(
    parcel_ids.elements_row_major_iter().map(|ids| ids.len() as u32),
    depo_count, depo_count);

  // plan the edges
  let mut edge_state = init_edge_state(depo_count as u32, cfg.truck.cap, p_mat);
  plan_edges_hub(&mut edge_state, airport as u32, &bar);
  plan_edges_all(&mut edge_state, &bar);
  let edge_plan = plan_edges(edge_state);

  let min_cost =
    edge_plan.min_edge_count as u64 * cfg.truck.go_cost +
    edge_plan.parcel_count as u64 * cfg.truck.transfer_cost;

  // plan the journeys
  let journey_plan = plan_journeys(&JourneyProblem {
    vertex_count: depo_count as u32,
    stage_count: 2,
    vehicle_vertices: problem.truck_depos,
    edges: &edge_plan.edges,
    constraints: &edge_plan.constraints,
  }, &bar);

  // plan the parcels
  let parcel_plan = plan_parcels(&ParcelProblem {
    vertex_ids: &problem.depo_ids,
    vehicle_ids: &problem.truck_ids,
    edges: &edge_plan.edges,
    legs: &journey_plan.legs,
    parcel_ids: &parcel_ids,
  }, &bar);

  let mut actions = parcel_plan.actions;
  assert_eq!(actions.len(), 2);
  let after_air_actions = actions.pop().unwrap();
  let before_air_actions = actions.pop().unwrap();
  let cost = sum_cost(&after_air_actions, &cfg.truck) +
    sum_cost(&before_air_actions, &cfg.truck);

  bar.finish_and_clear();
  CityPlan { before_air_actions, after_air_actions, cost, min_cost }
}

fn solve_air_problem(problem: AirProblem, cfg: &Config, bar: &ProgressBar) -> AirPlan {
  bar.set_message("initializing");

  let airport_count = problem.airport_ids.len();
  let airplane_count = problem.airplane_airports.len() as u32;

  let parcel_ids = problem.parcel_ids;
  let p_mat = Array2D::from_iter_row_major(
    parcel_ids.elements_row_major_iter().map(|ids| ids.len() as u32),
    airport_count, airport_count);

  let mut edge_state = init_edge_state(airport_count as u32, cfg.airplane.cap, p_mat);
  plan_edges_all(&mut edge_state, &bar);
  let edge_plan = plan_edges(edge_state);

  let min_cost =
    edge_plan.min_edge_count as u64 * cfg.airplane.go_cost +
    edge_plan.parcel_count as u64 * cfg.airplane.transfer_cost;

  let journey_plan = plan_journeys(&JourneyProblem {
    vertex_count: airport_count as u32,
    stage_count: 1,
    vehicle_vertices: problem.airplane_airports,
    edges: &edge_plan.edges,
    constraints: &edge_plan.constraints,
  }, &bar);

  let airplane_ids: Vec<_> = (0..airplane_count).collect();
  let parcel_plan = plan_parcels(&ParcelProblem {
    vertex_ids: &problem.airport_ids,
    vehicle_ids: &airplane_ids,
    edges: &edge_plan.edges,
    legs: &journey_plan.legs,
    parcel_ids: &parcel_ids,
  }, &bar);

  let mut actions = parcel_plan.actions;
  assert_eq!(actions.len(), 1);
  let air_actions = actions.pop().unwrap();
  let cost = sum_cost(&air_actions, &cfg.airplane);

  AirPlan { air_actions, cost, min_cost }
}

fn sum_cost(actions: &[Action], vehicle: &VehicleConfig) -> u64 {
  actions.iter().map(|a| match a {
    Action::Go { .. } => vehicle.go_cost,
    Action::Load { .. } => 0,
    Action::Unload { .. } => vehicle.transfer_cost,
  }).sum()
}

fn spinner_style() -> ProgressStyle {
  ProgressStyle::default_spinner()
    .template("{prefix:<12} {elapsed_precise} {spinner} {msg}")
    .tick_chars("-\\|/")
}

fn bar_style() -> ProgressStyle {
  ProgressStyle::default_bar()
    .template("{prefix:<12} {elapsed_precise} {bar:30} {pos}/{len} {msg}")
    .progress_chars("#>-")
}

