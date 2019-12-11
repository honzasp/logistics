use array2d::{Array2D};
use indicatif::{ProgressBar};
use fnv::{FnvHashMap, FnvHashSet};
use crate::{edge_plan::{Edge}, journey_plan::{Leg}};

#[derive(Debug)]
pub struct ParcelProblem<'p> {
  pub vertex_ids: &'p [u32],
  pub vehicle_ids: &'p [u32],
  pub edges: &'p [Edge],
  pub legs: &'p [Vec<Leg>],
  pub parcel_ids: &'p Array2D<Vec<u32>>,
}

#[derive(Debug)]
pub struct ParcelPlan {
  pub actions: Vec<Vec<Action>>,
}

#[derive(Debug, Clone)]
pub enum Action {
  Go { vehicle_id: u32, src_id: u32, tgt_id: u32 },
  Load { vehicle_id: u32, parcel_id: u32 },
  Unload { vehicle_id: u32, parcel_id: u32 },
}

struct State<'p> {
  problem: &'p ParcelProblem<'p>,
  stage: u32,
  vertex_vehicles: Vec<FnvHashSet<u32>>,
  unloaded_parcel_ids: Array2D<Vec<u32>>,
  loaded_parcel_ids: Array2D<Vec<u32>>,
  actions: Vec<Vec<Action>>,
}

/// Plans actions that load and unload parcels and move vehicles according to
/// the planned edges and legs.
pub fn plan_parcels(problem: &ParcelProblem, bar: &ProgressBar) -> ParcelPlan {
  let leg_count = problem.legs.iter().map(|l| l.len()).sum::<usize>();
  bar.reset();
  bar.set_message("planning parcels");
  bar.set_length(leg_count as u64);
  bar.set_draw_delta((leg_count/100) as u64);

  let mut state = init_state(problem);
  while state.stage < problem.legs.len() as u32 {
    for leg in problem.legs[state.stage as usize].iter() {
      plan_leg(&mut state, leg);
      bar.inc(1);
    }
    state.stage += 1;
  }
  ParcelPlan { actions: state.actions }
}

/// Plans actions corresponding to a single leg. We must ensure that the vehicle
/// has the right amount of parcels (by Load-ing and Unload-ing), then we can Go
/// and unload the parcels destined for the target.
fn plan_leg(state: &mut State, leg: &Leg) {
  let vehicle_id = state.problem.vehicle_ids[leg.vehicle as usize];

  // compute the number of parcels for each target from the edge cargo (or zero
  // if the leg does not correspond to any edge)
  let tgt_amounts: FnvHashMap<u32, u32> = leg.edge_idx
    .map(|edge_idx| &state.problem.edges[edge_idx as usize].cargo[..]).unwrap_or(&[])
    .iter().map(|edge_cargo| (edge_cargo.tgt, edge_cargo.amount)).collect();

  // ensure that we have the correct number of parcels. we must first unload and
  // then load to ensure that we never exceed the capacity of the vehicle
  for &can_load in [false, true].iter() {
    for tgt in 0..(state.problem.vertex_ids.len() as u32) {
      // ensure that we have the correct number of parcels going to tgt
      let tgt_amount = tgt_amounts.get(&tgt).cloned().unwrap_or(0);
      let loaded_amount = get_loaded(state, leg.vehicle, tgt).len() as u32;

      if can_load && tgt_amount > loaded_amount {
        // load more parcels
        for _ in 0..(tgt_amount - loaded_amount) {
          let parcel_id = plan_unloaded_parcel(state, leg.vehicle, leg.src, tgt);
          get_loaded(state, leg.vehicle, tgt).push(parcel_id);
          emit_action(state, Action::Load { vehicle_id, parcel_id });
        }
      } else if tgt_amount < loaded_amount {
        // unload some parcels
        for _ in 0..(loaded_amount - tgt_amount) {
          let parcel_id = get_loaded(state, leg.vehicle, tgt).pop().unwrap();
          get_unloaded(state, leg.src, tgt).push(parcel_id);
          emit_action(state, Action::Unload { vehicle_id, parcel_id });
        }
      }
    }
  }

  // go to the target vertex
  let src_id = state.problem.vertex_ids[leg.src as usize];
  let tgt_id = state.problem.vertex_ids[leg.tgt as usize];
  emit_action(state, Action::Go { vehicle_id, src_id, tgt_id });
  state.vertex_vehicles[leg.src as usize].remove(&leg.vehicle);
  state.vertex_vehicles[leg.tgt as usize].insert(leg.vehicle);

  // unload parcels destined for the target
  while let Some(parcel_id) = get_loaded(state, leg.vehicle, leg.tgt).pop() {
    emit_action(state, Action::Unload { vehicle_id, parcel_id });
  }
}

/// Plans action to acquire a parcel going from src to tgt to be loaded into the
/// given vehicle (so we cannot unload it from this vehicle).
fn plan_unloaded_parcel(state: &mut State, tgt_vehicle: u32, src: u32, tgt: u32) -> u32 {
  if let Some(parcel_id) = get_unloaded(state, src, tgt).pop() {
    // there is a parcel lying at src that is already unloaded
    return parcel_id;
  } else {
    // find a vehicle that is currently at src and has a parcel going to tgt
    let vehicles: Vec<_> = state.vertex_vehicles[src as usize].iter()
      .cloned().filter(|&v| v != tgt_vehicle).collect();
    for vehicle in vehicles {
      if let Some(parcel_id) = get_loaded(state, vehicle, tgt).pop() {
        let vehicle_id = state.problem.vehicle_ids[vehicle as usize];
        emit_action(state, Action::Unload { vehicle_id, parcel_id });
        return parcel_id;
      }
    }
    panic!("Ran out of parcels");
  }
}

fn init_state<'p>(problem: &'p ParcelProblem) -> State<'p> {
  let vertex_count = problem.vertex_ids.len();
  let vehicle_count = problem.vehicle_ids.len();
  State {
    problem,
    stage: 0,
    vertex_vehicles: vec![FnvHashSet::default(); vertex_count],
    unloaded_parcel_ids: problem.parcel_ids.clone(),
    loaded_parcel_ids: Array2D::filled_with(Vec::new(), vehicle_count, vertex_count),
    actions: vec![Vec::new(); problem.legs.len()],
  }
}

fn emit_action(state: &mut State, action: Action) {
  state.actions[state.stage as usize].push(action);
}

fn get_loaded<'s>(state: &'s mut State, vehicle: u32, tgt: u32) -> &'s mut Vec<u32> {
  &mut state.loaded_parcel_ids[(vehicle as usize, tgt as usize)]
}

fn get_unloaded<'s>(state: &'s mut State, src: u32, tgt: u32) -> &'s mut Vec<u32> {
  &mut state.unloaded_parcel_ids[(src as usize, tgt as usize)]
}
