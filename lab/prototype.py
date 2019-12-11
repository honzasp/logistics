import collections
import numpy as np

plane_cap = 30
city_count = 20
crate_count = int(0.7*plane_cap * city_count**2)
plane_count = 2

Edge = collections.namedtuple("Edge", ["i", "j", "cargo"])
PlaneEdge = collections.namedtuple("PlaneEdge", ["plane_idx", "i", "j", "cargo"])

def gen_crates_planes():
    crates = np.zeros((city_count, city_count), dtype=np.int)
    for _ in range(crate_count):
        i = np.random.randint(city_count)
        j = np.random.randint(city_count)
        if i != j:
            crates[i, j] += 1

    planes = [np.random.randint(city_count) for __ in range(plane_count)]

    return crates, planes

def plan_edges(crates):
    ij_order = [(i, j) for i in range(city_count) for j in range(city_count)]
    ij_order = sorted(ij_order, reverse=True, key=lambda ij: crates[ij])

    edges = []
    constraints = set()

    def add_edge(i, j, amount):
        assert amount <= plane_cap
        cargo = np.zeros(city_count, dtype=np.int)
        cargo[j] += amount
        edges.append(Edge(i, j, cargo))

    def add_constraint(ei, ej):
        assert (ej, ei) not in constraints
        assert ei != ej
        if (ei, ej) in constraints: return

        constraints.add((ei, ej))
        for (ek, el) in set(constraints):
            if ek == ej: add_constraint(ei, el)
            if el == ei: add_constraint(ek, ej)

    def find_path(path_i, path_j):
        paths = np.full(city_count, None, dtype=np.object)
        paths[path_i] = []
        visited = collections.deque([path_i])
        while visited:
            visited_i = visited.popleft()
            path_to_i = paths[visited_i]

            for edge_idx, edge in enumerate(edges):
                if edge.i != visited_i: continue
                if paths[edge.j] is not None: continue
                if np.sum(edge.cargo) >= plane_cap: continue

                can_be_used = True
                for path_edge_idx in path_to_i:
                    if (edge_idx, path_edge_idx) in constraints:
                        can_be_used = False
                if can_be_used:
                    paths[edge.j] = path_to_i + [edge_idx]
                    visited.append(edge.j)

        return paths[path_j]

    for (i, j) in ij_order:
        while crates[i, j] >= plane_cap:
            add_edge(i, j, plane_cap)
            crates[i, j] -= plane_cap

        while crates[i, j] > 0:
            ij_path = find_path(i, j)
            if ij_path is None: break
            path_cap = min(plane_cap - np.sum(edges[idx].cargo) for idx in ij_path)
            amount = min(path_cap, crates[i, j])
            for edge_idx in ij_path:
                edges[edge_idx].cargo[j] += amount
            for prev_edge_idx, next_edge_idx in zip(ij_path, ij_path[1:]):
                add_constraint(prev_edge_idx, next_edge_idx)
            crates[i, j] -= amount

        if crates[i, j] > 0:
            add_edge(i, j, crates[i, j])
            crates[i, j] = 0

    return edges, constraints

def plan_planes(edges, constraints, planes):
    all_out_edges = [set() for __ in range(city_count)]
    all_in_edges = [set() for __ in range(city_count)]
    available_out_edges = [set() for __ in range(city_count)]
    available_in_edges = [set() for __ in range(city_count)]
    visited_edges = set()

    def is_available(idx):
        return all(ei in visited_edges for (ei, ej) in constraints if ej == idx)

    def make_available(idx):
        edge = edges[idx]
        available_out_edges[edge.i].add(idx)
        available_in_edges[edge.j].add(idx)

    for idx, edge in enumerate(edges):
        if is_available(idx):
            make_available(idx)
        all_out_edges[edge.i].add(idx)
        all_in_edges[edge.j].add(idx)

    plane_edges = []
    plane_pos = np.array(planes)

    def visit_edge(idx):
        edge = edges[idx]
        available_out_edges[edge.i].remove(idx)
        available_in_edges[edge.j].remove(idx)
        visited_edges.add(idx)

        for unlock_idx in all_out_edges[edge.j]:
            is_visited = unlock_idx in visited_edges
            is_already_available = unlock_idx in available_out_edges[edge.j]
            if not is_visited and not is_already_available and is_available(unlock_idx):
                make_available(unlock_idx)

    def extend_journey(plane_idx):
        i = plane_pos[plane_idx]
        extended = False
        while True:
            out_edges = list(available_out_edges[i])
            if len(out_edges) > 0:
                idx = out_edges[np.random.randint(len(out_edges))]
                visit_edge(idx)

                edge = edges[idx]
                plane_edges.append(PlaneEdge(plane_idx, edge.i, edge.j, edge.cargo))
                extended = True
                i = edge.j
            else:
                break
        plane_pos[plane_idx] = i
        return extended

    while True:
        while True:
            extended = False
            for plane_idx in range(len(planes)):
                extended |= extend_journey(plane_idx)
            if not extended: break

        jump_js = [j for j in range(city_count) if len(available_out_edges[j]) > 0]
        if len(jump_js) == 0: break

        jump_j = max(jump_js, key=lambda j:
            len(available_out_edges[j]) - len(available_in_edges[j]))
        jump_plane_idx, jump_i = min(enumerate(list(plane_pos)), key=lambda ii:
            len(all_out_edges[ii[1]] - visited_edges))

        plane_pos[jump_plane_idx] = jump_j
        plane_edges.append(PlaneEdge(
            jump_plane_idx, jump_i, jump_j, np.zeros(city_count, dtype=np.int)))

    assert visited_edges == set(range(len(edges)))
    return plane_edges



np.random.seed(42)
crates, planes = gen_crates_planes()
print(crates)

min_landings = np.sum((crates.sum(axis=1) + plane_cap - 1)//plane_cap)
min_takeoffs = np.sum((crates.sum(axis=0) + plane_cap - 1)//plane_cap)
min_edges = max(min_landings, min_takeoffs)
print(min_edges)

edges, constraints = plan_edges(crates)
#for edge in edges: print(edge)
print(len(edges))
print(f"{len(edges)/min_edges:.2f}")

plane_edges = plan_planes(edges, constraints, planes)
#for edge in plane_edges: print(edge)
print(len(plane_edges))
print(f"{len(plane_edges)/min_edges:.2f} ({len(plane_edges)/len(edges):.2f})")
