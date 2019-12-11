use indicatif::{ProgressBar};
use fnv::{FnvHashSet};
use crate::{constraints::{Constraints}, edge_plan::{Edge}};

#[derive(Debug)]
pub struct JourneyProblem<'p> {
  pub vertex_count: u32,
  pub stage_count: u32,
  pub vehicle_vertices: Vec<u32>,
  pub edges: &'p [Edge],
  pub constraints: &'p Constraints,
}

#[derive(Debug)]
pub struct JourneyPlan {
  pub legs: Vec<Vec<Leg>>,
}

#[derive(Debug, Clone)]
pub struct Leg {
  pub vehicle: u32,
  pub src: u32,
  pub tgt: u32,
  pub edge_idx: Option<u32>,
}

struct State<'p> {
  problem: &'p JourneyProblem<'p>,
  stage: u32,
  vehicle_vertices: Vec<u32>,
  available_out_edges: Vec<FnvHashSet<u32>>,
  available_in_edge_counts: Vec<u32>,
  visited_edges: FnvHashSet<u32>,
  legs: Vec<Vec<Leg>>,
}

/// Plan a journey for each vehicle so that the vehicles collectively visit all
/// edges in the problem.
pub fn plan_journeys(problem: &JourneyProblem, bar: &ProgressBar) -> JourneyPlan {
  bar.reset();
  bar.set_message("planning journeys");

  let mut state = init_state(problem);
  let vehicle_count = problem.vehicle_vertices.len() as u32;

  bar.set_length(problem.edges.len() as u64);
  bar.set_draw_delta((problem.edges.len()/100) as u64);
  loop {
    // try to extend the journey of each vehicle, following the available
    // edges
    loop {
      let extended = (0..vehicle_count).any(|vehicle|
        extend_journey(&mut state, vehicle, bar));
      if !extended { break }
    }

    // all vehicles are stuck in their vertices, so a vehicle must jump to
    // another vertex without following an edge

    // compute the best jump target according to a heuristic
    let jump_tgt = (0..problem.vertex_count)
      .filter(|&vertex| !state.available_out_edges[vertex as usize].is_empty())
      .max_by_key(|&vertex| get_available_deg(&state, vertex));

    if let Some(jump_tgt) = jump_tgt {
      // compute the best jump source (vehicle and vertex) according to a
      // heuristic
      let (jump_vehicle, jump_src) = (0..vehicle_count)
        .map(|vehicle| (vehicle, state.vehicle_vertices[vehicle as usize]))
        .min_by_key(|&(_, vertex)| get_available_deg(&state, vertex))
        .expect("At least one vehicle is needed");

      // perform the jump
      state.legs[state.stage as usize].push(Leg {
        vehicle: jump_vehicle,
        src: jump_src, tgt: jump_tgt,
        edge_idx: None,
      });
      state.vehicle_vertices[jump_vehicle as usize] = jump_tgt;
    } else if state.stage < problem.stage_count {
      // there are no available edges, but we can increase stage and unlock more
      // edges
      let next_stage = state.stage + 1;
      make_stage_available(&mut state, next_stage);
      state.stage = next_stage;
    } else {
      // no edges are available and no more stages can be unlocked
      break;
    }
  }

  assert!(state.visited_edges.len() == problem.edges.len());
  JourneyPlan { legs: state.legs }
}

/// Creates an initial state for solving the problem.
fn init_state<'p>(problem: &'p JourneyProblem) -> State<'p> {
  let mut state = State {
    problem,
    stage: 0,
    vehicle_vertices: problem.vehicle_vertices.clone(),
    available_out_edges: vec![FnvHashSet::default(); problem.vertex_count as usize],
    available_in_edge_counts: vec![0; problem.vertex_count as usize],
    visited_edges: FnvHashSet::default(),
    legs: vec![Vec::new(); problem.stage_count as usize],
  };

  for edge_idx in 0..(problem.edges.len() as u32) {
    if is_stage_available(&state, edge_idx) && is_unconstrained(&state, edge_idx) {
      add_available_edge(&mut state, edge_idx);
    }
  }

  state
}

/// Greedily follow available edges with the given vehicle until a vertex with
/// no outgoing available edges is reached.
fn extend_journey(state: &mut State, vehicle: u32, bar: &ProgressBar) -> bool {
  let mut vertex = state.vehicle_vertices[vehicle as usize];
  let mut extended = false;

  loop {
    // pick an available edge using a heuristic
    let edge_tgt = state.available_out_edges[vertex as usize].iter()
      .map(|&edge_idx| (edge_idx, get_edge(state, edge_idx).tgt))
      .max_by_key(|&(_, tgt)| get_available_deg(state, tgt));

    if let Some((edge_idx, tgt)) = edge_tgt {
      // mark the edge as visited
      visit_edge(state, edge_idx);
      bar.inc(1);

      // follow the edge
      state.legs[state.stage as usize].push(Leg {
        vehicle, src: vertex, tgt, edge_idx: Some(edge_idx),
      });
      vertex = tgt;
      extended = true;
    } else {
      break
    }
  }

  state.vehicle_vertices[vehicle as usize] = vertex;
  extended
}

/// Marks all edges with the given stage that are not constrained as available.
fn make_stage_available(state: &mut State, stage: u32) {
  for (edge_idx, edge) in state.problem.edges.iter().enumerate() {
    if let Some(edge_stage) = edge.stage {
      if edge_stage == stage && is_unconstrained(state, edge_idx as u32) {
        add_available_edge(state, edge_idx as u32);
      } else if edge_stage < stage {
        assert!(state.visited_edges.contains(&(edge_idx as u32)));
      }
    }
  }
}

/// Mark the edge as visited: remove it from available and make available other
/// edges that depend on it.
fn visit_edge(state: &mut State, edge_idx: u32) {
  remove_available_edge(state, edge_idx);
  state.visited_edges.insert(edge_idx);
  for after_idx in state.problem.constraints.successors(edge_idx) {
    if is_stage_available(state, after_idx) && is_unconstrained(state, after_idx) {
      add_available_edge(state, after_idx);
    }
  }
}

/// Adds the edge to the available subgraph.
fn add_available_edge(state: &mut State, edge_idx: u32) {
  let edge = get_edge(state, edge_idx);
  assert!(state.available_out_edges[edge.src as usize].insert(edge_idx));
  state.available_in_edge_counts[edge.tgt as usize] += 1;
}

/// Removes the edge from the available subgraph.
fn remove_available_edge(state: &mut State, edge_idx: u32) {
  let edge = get_edge(state, edge_idx);
  assert!(state.available_out_edges[edge.src as usize].remove(&edge_idx));
  state.available_in_edge_counts[edge.tgt as usize] -= 1;
}

/// Decides whether all edges that this edge depends on are visited.
fn is_unconstrained(state: &State, edge_idx: u32) -> bool {
  state.problem.constraints.predecessors(edge_idx)
    .all(|prev_idx| state.visited_edges.contains(&prev_idx))
}

/// Decides whether the stage of the given edge is available. This is true if
/// edge has no stage or the edge stage matches state.stage.
fn is_stage_available(state: &State, edge_idx: u32) -> bool {
  get_edge(state, edge_idx).stage.map(|stage| state.stage == stage).unwrap_or(true)
}

/// Returns out degree - in degree of the vertex in the graph of available
/// edges.
fn get_available_deg(state: &State, vertex: u32) -> i32 {
  let out_deg = state.available_out_edges[vertex as usize].len() as i32;
  let in_deg = state.available_in_edge_counts[vertex as usize] as i32;
  out_deg - in_deg
}

fn get_edge<'p>(state: &State<'p>, edge_idx: u32) -> &'p Edge {
  &state.problem.edges[edge_idx as usize]
}
