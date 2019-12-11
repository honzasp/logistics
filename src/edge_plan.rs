use array2d::{Array2D};
use fnv::{FnvHashSet};
use std::{cmp};
use indicatif::{ProgressBar};
use crate::{constraints::Constraints};

#[derive(Debug)]
pub struct EdgePlan {
  pub edges: Vec<Edge>,
  pub constraints: Constraints,
  pub min_edge_count: u32,
  pub parcel_count: u32,
}

#[derive(Debug)]
pub struct Edge {
  pub src: u32,
  pub tgt: u32,
  pub free_cap: u32,
  pub cargo: Vec<EdgeCargo>,
  pub stage: Option<u32>,
}

#[derive(Debug)]
pub struct EdgeCargo {
  pub tgt: u32,
  pub amount: u32,
}

#[derive(Debug)]
pub struct EdgeState {
  p_mat: Array2D<u32>,
  edges: Vec<Edge>,
  constraints: Constraints,
  free_out_edges: Vec<FnvHashSet<u32>>,
  vertex_count: u32,
  edge_cap: u32,
  min_edge_count: u32,
  parcel_count: u32,
}

/// Initializes an EdgeState structure that is used to plan edges.
pub fn init_edge_state(vertex_count: u32, edge_cap: u32,
  p_mat: Array2D<u32>) -> EdgeState
{
  // calculate lower bound on the number of edges
  let min_out_edges = (0..vertex_count).map(|i| {
      let out_count = (0..vertex_count).filter(|&j| j != i).map(|j|
        p_mat[(i as usize, j as usize)]
      ).sum::<u32>();
      (out_count + edge_cap - 1) / edge_cap
    }).sum();
  let min_in_edges = (0..vertex_count).map(|j| {
      let in_count = (0..vertex_count).filter(|&i| i != j).map(|i|
        p_mat[(i as usize, j as usize)]
      ).sum::<u32>();
      (in_count + edge_cap - 1) / edge_cap
    }).sum();
  let min_edge_count = cmp::max(min_out_edges, min_in_edges);

  // calculate number of parcels
  let parcel_count = (0..vertex_count).map(|i|
      (0..vertex_count).filter(|&j| j != i).map(|j|
        p_mat[(i as usize, j as usize)]
      ).sum::<u32>()
    ).sum();

  EdgeState {
    p_mat,
    edges: Vec::new(),
    constraints: Constraints::new(),
    free_out_edges: vec![FnvHashSet::default(); vertex_count as usize],
    vertex_count, edge_cap, min_edge_count, parcel_count,
  }
}

/// Converts an EdgeState which contains planned edges and constraints to a
/// final plan.
pub fn plan_edges(state: EdgeState) -> EdgePlan {
  EdgePlan {
    edges: state.edges,
    constraints: state.constraints,
    min_edge_count: state.min_edge_count,
    parcel_count: state.parcel_count,
  }
}

/// Plans all edges going to hub with stage Some(0), then all edges going from
/// hub with stage Some(1). This is used to plan trucks going to and from the
/// airport and attempts to minimize the number of journeys required.
pub fn plan_edges_hub(state: &mut EdgeState, hub: u32, bar: &ProgressBar) {
  let stage0_begin = state.edges.len();
  plan_hub_dir(state, hub, true, 0, bar);
  let stage1_begin = state.edges.len();
  plan_hub_dir(state, hub, false, 1, bar);
  let stage1_end = state.edges.len();

  bar.reset();
  bar.set_message("planning edges (hubward-rimward constraints)");
  bar.set_length((stage1_begin - stage0_begin) as u64);
  bar.set_draw_delta(((stage1_begin - stage0_begin)/100) as u64);
  for i in stage0_begin..stage1_begin {
    for j in stage1_begin..stage1_end {
      state.constraints.add_before(i as u32, j as u32);
    }
    bar.inc(1);
  }
}

/// Plan all edges that go to hub (if hubward is true) or from hub (if hubward
/// is false).
fn plan_hub_dir(state: &mut EdgeState, hub: u32,
  hubward: bool, stage: u32, bar: &ProgressBar)
{
  bar.reset();
  bar.set_message(if hubward { "planning edges (hubward)" }
    else { "planning edges (rimward)" });

  fn src_tgt(vertex: u32, hub: u32, hubward: bool) -> (u32, u32) {
    if hubward { (vertex, hub) } else { (hub, vertex) }
  }

  // compute the heuristic order of vertices
  let mut vertices = (0..state.vertex_count)
    .filter(|&v| v != hub).collect::<Vec<_>>();
  vertices.sort_unstable_by(|&v1, &v2| {
    let (src1, tgt1) = src_tgt(v1, hub, hubward);
    let (src2, tgt2) = src_tgt(v2, hub, hubward);
    let p1 = state.p_mat[(src1 as usize, tgt1 as usize)] % state.edge_cap;
    let p2 = state.p_mat[(src2 as usize, tgt2 as usize)] % state.edge_cap;
    p2.cmp(&p1)
  });
  bar.set_length(vertices.len() as u64);

  struct HubPath {
    vertex: u32,
    edges: Vec<u32>,
    free_cap: u32,
  }
  let mut hub_paths: Vec<HubPath> = vec![];

  for vertex in vertices {
    let (src, tgt) = src_tgt(vertex, hub, hubward);
    let mut amount = state.p_mat[(src as usize, tgt as usize)];

    // add fully saturated edges src -> tgt
    while amount >= state.edge_cap {
      add_edge(state, src, tgt, state.edge_cap, Some(stage));
      amount -= state.edge_cap;
    }

    if amount > 0 {
      let path_i = (0..hub_paths.len())
        .filter(|&i| hub_paths[i].free_cap >= amount).next();
      if let Some(path_i) = path_i {
        // extend an existing path
        let mut hub_path = hub_paths.swap_remove(path_i);

        // add an edge that extends the the hub path
        let (add_src, add_tgt) = src_tgt(vertex, hub_path.vertex, hubward);
        let added_idx = add_edge(state, add_src, add_tgt, 0, Some(stage));

        // send parcels along the edges in the path
        for &edge_idx in hub_path.edges.iter() {
          send_along_edge(state, edge_idx, tgt, amount);
        }
        send_along_edge(state, added_idx, tgt, amount);

        // add a constraint linking the added edge to the path
        let (before_idx, after_idx) = src_tgt(
          added_idx, *hub_path.edges.last().unwrap(), hubward);
        state.constraints.add_before(before_idx, after_idx);

        if hub_path.free_cap > amount {
          // update the path and add it back to hub_paths
          hub_path.vertex = vertex;
          hub_path.edges.push(added_idx);
          hub_path.free_cap -= amount;
          hub_paths.push(hub_path);
        }
      } else {
        // start a new hub path
        let edge_idx = add_edge(state, src, tgt, amount, Some(stage));
        let free_cap = state.edge_cap - amount;
        hub_paths.push(HubPath { vertex, edges: vec![edge_idx], free_cap });
      }
    }

    state.p_mat[(src as usize, tgt as usize)] = 0;
    bar.inc(1);
  }
}

/// Plan all remaining edges between vertices, using stage None.
pub fn plan_edges_all(state: &mut EdgeState, bar: &ProgressBar) {
  bar.reset();
  bar.set_message("planning edges");

  // compute the heuristic order of (src, tgt) pairs: decreasing by p_mat(src,
  // tgt)
  let mut src_tgts: Vec<_> = (0..state.vertex_count)
    .map(|src| (0..state.vertex_count).map(move |tgt| (src, tgt))).flatten()
    .filter(|&(src, tgt)| src != tgt && state.p_mat[(src as usize, tgt as usize)] > 0)
    .collect();
  src_tgts.sort_unstable_by(|&(src1, tgt1), &(src2, tgt2)| {
    let p1 = state.p_mat[(src1 as usize, tgt1 as usize)] % state.edge_cap;
    let p2 = state.p_mat[(src2 as usize, tgt2 as usize)] % state.edge_cap;
    p2.cmp(&p1)
  });

  bar.set_length(src_tgts.len() as u64);
  for (src, tgt) in src_tgts {
    let mut amount = state.p_mat[(src as usize, tgt as usize)];

    while amount >= state.edge_cap {
      // there are enough parcels to add fully saturated edges src -> tgt
      add_edge(state, src, tgt, state.edge_cap, None);
      amount -= state.edge_cap;
    }

    if amount > 0 {
      if let Some(path) = find_path(&state, src, tgt, amount) {
        // there is a path src -> tgt, send parcels along it
        augment_path(state, tgt, &path, amount);
      } else {
        // add an unsaturated edge src -> tgt
        add_edge(state, src, tgt, amount, None);
      }
    }

    state.p_mat[(src as usize, tgt as usize)] = 0;
    bar.inc(1);
  }
}

/// Applies an augmenting path by sending parcels along its edges.
fn augment_path(state: &mut EdgeState, tgt: u32, path: &[u32], amount: u32) {
  for (path_idx, &edge_idx) in path.iter().enumerate() {
    if path_idx > 0 {
      state.constraints.add_before(path[path_idx-1], path[path_idx]);
    }
    send_along_edge(state, edge_idx, tgt, amount);
  }
}

/// Adds a new edge src -> tgt and sends given amount of parcels along it.
fn add_edge(state: &mut EdgeState, src: u32, tgt: u32,
  amount: u32, stage: Option<u32>) -> u32
{
  assert!(amount <= state.edge_cap);
  let edge_idx = state.edges.len() as u32;

  state.edges.push(Edge {
    src, tgt,
    free_cap: state.edge_cap - amount,
    cargo: vec![EdgeCargo { tgt, amount }],
    stage,
  });
  state.constraints.push();

  if amount < state.edge_cap {
    state.free_out_edges[src as usize].insert(edge_idx);
  }

  (state.edges.len() - 1) as u32
}

/// Sends given amount of parcels destined for tgt along the given edge.
fn send_along_edge(state: &mut EdgeState, edge_idx: u32, tgt: u32, amount: u32) {
  let edge = &mut state.edges[edge_idx as usize];
  assert!(edge.free_cap >= amount);

  let mut added = false;
  for edge_cargo in edge.cargo.iter_mut() {
    if edge_cargo.tgt == tgt {
      edge_cargo.amount += amount;
      added = true;
      break;
    }
  }
  if !added {
    edge.cargo.push(EdgeCargo { tgt, amount });
  }

  edge.free_cap -= amount;
  if edge.free_cap == 0 {
    state.free_out_edges[edge.src as usize].remove(&edge_idx);
  }
}

/// Attempts to find a free path from path_src to path_tgt with capacity at
/// least min_cap.
fn find_path(state: &EdgeState, path_src: u32, path_tgt: u32, min_cap: u32)
  -> Option<Vec<u32>>
{
  assert!(path_src != path_tgt);

  // check that edge_idx can be used to extend the path ending in vertex
  fn can_use_edge(state: &EdgeState, edges_to: &[u32],
    mut vertex: u32, edge_idx: u32) -> bool
  {
    loop {
      let prev_edge_idx = edges_to[vertex as usize];
      if prev_edge_idx == !0 { return true }
      if state.constraints.is_before(edge_idx, prev_edge_idx) { return false }
      vertex = state.edges[prev_edge_idx as usize].src;
    }
  }

  // runs a breadth-first search over edges, stopping when a path to path_tgt is
  // found
  let mut current_vertices = vec![path_src];
  let mut edges_to = vec![!0; state.vertex_count as usize];

  'bfs: while !current_vertices.is_empty() {
    let mut next_edges = current_vertices.into_iter()
      .flat_map(|vertex| state.free_out_edges[vertex as usize].iter().cloned())
      .filter(|&next_edge_idx| {
        let next_edge = &state.edges[next_edge_idx as usize];
        next_edge.tgt != path_src
          && edges_to[next_edge.tgt as usize] == !0
          && next_edge.free_cap >= min_cap
          && can_use_edge(state, &edges_to, next_edge.src, next_edge_idx)
      })
      .collect::<Vec<_>>();
    next_edges.sort_by_cached_key(|&next_edge_idx| {
      let pred_count = state.constraints.count_predecessors(next_edge_idx);
      let free_cap = state.edges[next_edge_idx as usize].free_cap;
      (pred_count, cmp::Reverse(free_cap))
    });

    let mut next_vertices = vec![];
    for next_edge_idx in next_edges {
      let next_vertex = state.edges[next_edge_idx as usize].tgt;
      if edges_to[next_vertex as usize] == !0 {
        edges_to[next_vertex as usize] = next_edge_idx;
        if next_vertex == path_tgt { break 'bfs }
        next_vertices.push(next_vertex);
      }
    }
    current_vertices = next_vertices;
  }

  // reconstruct the path to path_tgt from edges_to[]
  if edges_to[path_tgt as usize] != !0 {
    let mut path = Vec::new();
    let mut vertex = path_tgt;
    while vertex != path_src {
      let edge_idx = edges_to[vertex as usize];
      path.push(edge_idx);
      vertex = state.edges[edge_idx as usize].src;
    }
    path.reverse();
    Some(path)
  } else {
    None
  }
}
