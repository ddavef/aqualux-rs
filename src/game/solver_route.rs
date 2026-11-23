// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

// Phase 1 solver move router; verified by tests now and used by Editor/Auto-Par in Phase 3.
#![allow(dead_code)]

use crate::game::awards::Difficulty;
use crate::game::level_sim::LevelSim;
use crate::game::path::PathSegment;
use crate::game::solver::{MovableKind, PlayerMove, SolveResult, canonical_period};
use crate::game::types::{LevelDef, PieceShape};
use std::collections::{HashMap, VecDeque, hash_map::Entry};

pub(crate) struct RouteCtx<'a> {
    pub level: &'a LevelDef,
    pub path: Vec<PathSegment>,
    pub locked_rot: &'a [Option<u8>],
    pub locked_move: &'a [Option<MovableKind>],
    pub comp_id: &'a [Option<usize>],
    pub comp_cells: &'a [Vec<usize>],
    pub initial_move_kinds: &'a [Option<MovableKind>],
    pub initial_rot_dirs: &'a [u8],
    pub shapes: &'a [PieceShape],
}

pub(crate) fn route_and_verify(ctx: RouteCtx<'_>) -> Option<SolveResult> {
    let mut moves = Vec::new();
    let cols = ctx.level.cols;

    // 1. Emit rotation clicks for visited `can_rotate` cells.
    for (idx, target_opt) in ctx.locked_rot.iter().enumerate() {
        let Some(target) = *target_opt else {
            continue;
        };
        let period = canonical_period(ctx.shapes[idx]);
        let clicks = if period <= 1 {
            0
        } else {
            let init = ctx.initial_rot_dirs[idx] % period;
            let tgt = target % period;
            (tgt + period - init) % period
        };
        let (x, y) = (idx % cols, idx / cols);
        for _ in 0..clicks {
            moves.push(PlayerMove::Rotate { x, y });
        }
    }

    // 2. Route movable tiles within each connected component.
    for (cid, cells) in ctx.comp_cells.iter().enumerate() {
        let has_targets = cells.iter().any(|&c| ctx.locked_move[c].is_some());
        if !has_targets {
            continue;
        }
        route_component(
            cols,
            cid,
            cells,
            ctx.comp_id,
            ctx.initial_move_kinds,
            ctx.locked_move,
            &mut moves,
        )?;
    }

    // 3. Replay move sequence headlessly on `LevelSim` to verify 100% parity.
    let pipes_filled = verify_moves_on_sim(ctx.level, &moves)?;
    let move_count = moves.len().max(1) as f32;
    let pipes_f = pipes_filled.max(1) as f32;

    let suggested_par_moves = (move_count * 1.25).ceil() as u32;
    let suggested_par_pipes = pipes_filled.max(1);
    let suggested_par_time_ms = ((move_count * 1.6 + pipes_f * 0.4) * 1000.0)
        .round()
        .clamp(10_000.0, 300_000.0) as u32;

    Some(SolveResult {
        path: ctx.path,
        moves,
        pipes_filled,
        suggested_par_moves,
        suggested_par_pipes,
        suggested_par_time_ms,
        verified_by_sim: true,
    })
}

fn route_component(
    cols: usize,
    cid: usize,
    cells: &[usize],
    comp_id: &[Option<usize>],
    initial_move_kinds: &[Option<MovableKind>],
    locked_move: &[Option<MovableKind>],
    moves: &mut Vec<PlayerMove>,
) -> Option<()> {
    let mut current: HashMap<usize, MovableKind> = HashMap::with_capacity(cells.len());
    let mut unlocked: Vec<usize> = cells.to_vec();
    for &c in cells {
        current.insert(c, initial_move_kinds[c]?);
    }

    while !unlocked.is_empty() {
        let mut demand: HashMap<MovableKind, usize> = HashMap::new();
        let mut supply: HashMap<MovableKind, usize> = HashMap::new();
        for &c in &unlocked {
            *supply.entry(current[&c]).or_insert(0) += 1;
            if let Some(req) = locked_move[c] {
                *demand.entry(req).or_insert(0) += 1;
            }
        }

        // First lock any tree leaves that already hold their exact target or are unneeded.
        let leaves = spanning_tree_leaves(cols, cid, comp_id, &unlocked);
        if let Some(&ready_leaf) = leaves.iter().find(|&&v| {
            if let Some(req) = locked_move[v] {
                current[&v] == req
            } else {
                let kind = current[&v];
                supply.get(&kind).copied().unwrap_or(0) > demand.get(&kind).copied().unwrap_or(0)
            }
        }) {
            unlocked.retain(|&u| u != ready_leaf);
            continue;
        }

        // Prefer a leaf that needs a specific target piece; otherwise free up a needed piece on a don't-care leaf.
        let leaf = leaves
            .iter()
            .copied()
            .find(|&v| locked_move[v].is_some())
            .or_else(|| leaves.first().copied())?;

        let src = if let Some(req) = locked_move[leaf] {
            bfs_nearest_in_unlocked(cols, leaf, &unlocked, |u| current[&u] == req)?
        } else {
            bfs_nearest_in_unlocked(cols, leaf, &unlocked, |u| {
                let kind = current[&u];
                supply.get(&kind).copied().unwrap_or(0) > demand.get(&kind).copied().unwrap_or(0)
            })?
        };

        let path = bfs_path_in_unlocked(cols, src, leaf, &unlocked)?;
        for win in path.windows(2) {
            let a = win[0];
            let b = win[1];
            let ka = current[&a];
            let kb = current[&b];
            current.insert(a, kb);
            current.insert(b, ka);
            moves.push(PlayerMove::Swap {
                a: (a % cols, a / cols),
                b: (b % cols, b / cols),
            });
        }
        unlocked.retain(|&u| u != leaf);
    }

    Some(())
}

fn spanning_tree_leaves(
    cols: usize,
    cid: usize,
    comp_id: &[Option<usize>],
    unlocked: &[usize],
) -> Vec<usize> {
    if unlocked.len() <= 1 {
        return unlocked.to_vec();
    }
    let root = unlocked[0];
    let mut visited = HashMap::with_capacity(unlocked.len());
    let mut has_child = HashMap::with_capacity(unlocked.len());
    let mut queue = VecDeque::new();
    visited.insert(root, true);
    queue.push_back(root);

    while let Some(u) = queue.pop_front() {
        for nb in unlocked_neighbors(cols, u, cid, comp_id, unlocked) {
            if let Entry::Vacant(e) = visited.entry(nb) {
                e.insert(true);
                has_child.insert(u, true);
                queue.push_back(nb);
            }
        }
    }

    unlocked
        .iter()
        .copied()
        .filter(|u| !has_child.get(u).copied().unwrap_or(false))
        .collect()
}

fn bfs_nearest_in_unlocked(
    cols: usize,
    target: usize,
    unlocked: &[usize],
    pred: impl Fn(usize) -> bool,
) -> Option<usize> {
    let mut seen = Vec::with_capacity(unlocked.len());
    let mut queue = VecDeque::new();
    seen.push(target);
    queue.push_back(target);

    while let Some(u) = queue.pop_front() {
        if pred(u) {
            return Some(u);
        }
        for nb in neighbors_in_list(cols, u, unlocked) {
            if !seen.contains(&nb) {
                seen.push(nb);
                queue.push_back(nb);
            }
        }
    }
    None
}

fn bfs_path_in_unlocked(
    cols: usize,
    src: usize,
    dst: usize,
    unlocked: &[usize],
) -> Option<Vec<usize>> {
    let mut parent: HashMap<usize, usize> = HashMap::with_capacity(unlocked.len());
    let mut queue = VecDeque::new();
    parent.insert(src, src);
    queue.push_back(src);

    while let Some(u) = queue.pop_front() {
        if u == dst {
            let mut path = Vec::new();
            let mut cur = dst;
            while cur != src {
                path.push(cur);
                cur = parent[&cur];
            }
            path.push(src);
            path.reverse();
            return Some(path);
        }
        for nb in neighbors_in_list(cols, u, unlocked) {
            if let Entry::Vacant(e) = parent.entry(nb) {
                e.insert(u);
                queue.push_back(nb);
            }
        }
    }
    None
}

fn unlocked_neighbors(
    cols: usize,
    idx: usize,
    cid: usize,
    comp_id: &[Option<usize>],
    unlocked: &[usize],
) -> Vec<usize> {
    let x = idx % cols;
    let y = idx / cols;
    let rows = comp_id.len() / cols;
    let mut out = Vec::with_capacity(4);
    for (dx, dy) in [(0i32, -1i32), (1, 0), (0, 1), (-1, 0)] {
        let nx = x as i32 + dx;
        let ny = y as i32 + dy;
        if nx >= 0 && ny >= 0 && (nx as usize) < cols && (ny as usize) < rows {
            let nidx = ny as usize * cols + nx as usize;
            if comp_id[nidx] == Some(cid) && unlocked.contains(&nidx) {
                out.push(nidx);
            }
        }
    }
    out
}

fn neighbors_in_list(cols: usize, idx: usize, unlocked: &[usize]) -> Vec<usize> {
    let x = idx % cols;
    let y = idx / cols;
    let mut out = Vec::with_capacity(4);
    for &cand in unlocked {
        let cx = cand % cols;
        let cy = cand / cols;
        if x.abs_diff(cx) + y.abs_diff(cy) == 1 {
            out.push(cand);
        }
    }
    out
}

fn verify_moves_on_sim(level: &LevelDef, moves: &[PlayerMove]) -> Option<u32> {
    let mut sim = LevelSim::new(level.clone(), Difficulty::Easy);
    sim.start_level();
    if !sim.level.tutorials.is_empty() {
        sim.skip_tutorials();
    }
    for mv in moves {
        match *mv {
            PlayerMove::Rotate { x, y } => {
                sim.click_tile(x, y);
            }
            PlayerMove::Swap { a, b } => {
                sim.try_swap_drag(a, b);
            }
        }
        let mut guard = 0;
        while sim.is_acting() && guard < 30 {
            sim.update(0.1);
            guard += 1;
        }
    }

    sim.fast_forward();
    for _ in 0..2000 {
        if sim.finished {
            break;
        }
        sim.update(1.0 / 30.0);
    }

    (sim.won == Some(true)).then_some(sim.pipes_filled)
}
