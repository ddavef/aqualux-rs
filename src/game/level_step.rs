// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::game::awards::Difficulty;
use crate::game::level_sim::{LevelSim, SimEvent};
use crate::game::path::{TilePathHighlight, find_matching_portal, trace_highlighted_segments};
use crate::game::scoring::calculate_score;
use crate::game::tile::collect_swap_eligible;
use crate::game::types::{FlowTimerState, PieceType, Side};

impl LevelSim {
    pub fn get_swap_eligible(&self, pos: (usize, usize)) -> Vec<(usize, usize)> {
        collect_swap_eligible(&self.tiles, self.level.cols, self.level.rows, pos, true)
    }

    pub fn clear_all_path_highlights(&mut self) {
        for row in &mut self.tiles {
            for tile in row {
                tile.path_hl = None;
            }
        }
    }

    pub(crate) fn recompute_path(&mut self, initial: bool, events: &mut Vec<SimEvent>) {
        let segs = trace_highlighted_segments(&self.tiles, &self.level);
        self.path_complete = segs.iter().any(|s| {
            self.tiles
                .get(s.y)
                .and_then(|r| r.get(s.x))
                .is_some_and(|t| t.piece_type == PieceType::Drain)
        });
        if initial {
            return;
        }
        if segs
            .iter()
            .any(|s| self.swapping_tiles.contains(&(s.x, s.y)))
        {
            self.hilight_seq = self.hilight_seq.wrapping_add(1).max(1);
            let hold = self.active_tutorial().is_some_and(|t| t.hold_hilight);
            events.push(SimEvent::PlaySfx("SwishSound"));
            for s in segs {
                let tile = &mut self.tiles[s.y][s.x];
                let rot_back = (4 - (tile.direction % 4)) % 4;
                let le = s.entry.rotate_cw(rot_back);
                let lx = s.exit.rotate_cw(rot_back);
                if let Some(hl) = &mut tile.path_hl {
                    hl.add_segment(le, lx, self.hilight_seq, hold);
                } else {
                    tile.path_hl = Some(TilePathHighlight {
                        seq: self.hilight_seq,
                        segs: vec![(le, lx)],
                        elapsed: 0.0,
                        hold,
                    });
                }
            }
        }
    }

    pub(crate) fn tutorial_allows_click(&self, pos: (usize, usize)) -> bool {
        let Some(tut) = self.active_tutorial() else {
            return true;
        };
        if !tut.wait_for_drag.is_empty() {
            return tut.wait_for_drag.contains(&pos);
        }
        if let Some(ws) = tut.wait_for_select {
            return pos == ws;
        }
        if !tut.wait_for_swap.is_empty() {
            return tut.wait_for_swap.contains(&pos);
        }
        false
    }

    pub(crate) fn tutorial_allows_drag(&self, from: (usize, usize), to: (usize, usize)) -> bool {
        let Some(tut) = self.active_tutorial() else {
            return true;
        };
        if !tut.wait_for_drag.is_empty() {
            return tut.wait_for_drag.contains(&from) && tut.wait_for_drag.contains(&to);
        }
        if !tut.wait_for_swap.is_empty() {
            return tut.wait_for_swap.contains(&from) && tut.wait_for_swap.contains(&to);
        }
        false
    }
    pub(crate) fn on_swap_complete(&mut self, events: &mut Vec<SimEvent>) {
        self.moves += 1;
        self.recompute_path(false, events);

        let completed_swap = std::mem::take(&mut self.swapping_tiles);
        if let Some(tut) = self.active_tutorial().cloned() {
            let advance = if !tut.wait_for_drag.is_empty() {
                tut.wait_for_drag.iter().any(|p| completed_swap.contains(p))
            } else if !tut.wait_for_swap.is_empty() {
                tut.wait_for_swap.iter().all(|p| completed_swap.contains(p))
            } else {
                false
            };
            if advance {
                events.extend(self.next_tutorial());
            }
        }

        let deferred = std::mem::take(&mut self.pending_exits);
        if !deferred.is_empty() {
            self.handle_completed_exits(deferred, events);
        }
    }

    pub(crate) fn advance_slimes(&mut self, dt: f32) {
        for row in &mut self.tiles {
            for tile in row {
                if !tile.slime_playing {
                    continue;
                }
                if let Some(sa) = &mut tile.slime_anim {
                    *sa += dt;
                    if *sa * 15.0 >= 25.0 {
                        tile.slime_playing = false;
                        tile.slime_done = true;
                        *sa = if tile.piece_type == PieceType::Portal {
                            0.0
                        } else {
                            25.0 / 15.0
                        };
                    }
                }
            }
        }
    }

    pub(crate) fn tick_timers(&mut self, dt: f32, events: &mut Vec<SimEvent>) {
        let fast_nozzle = self.is_accelerated || self.current_speed >= 15.0;
        let mut ready_sources = false;
        let mut ready_portals = Vec::new();
        for y in 0..self.level.rows {
            for x in 0..self.level.cols {
                let ptype = self.tiles[y][x].piece_type;
                let Some(t) = &mut self.tiles[y][x].timer else {
                    continue;
                };
                t.age += dt;
                if t.fading {
                    t.fade_elapsed += dt;
                    if t.fade_elapsed >= 1.0 {
                        self.tiles[y][x].timer = None;
                    }
                    continue;
                }
                t.remaining -= dt;
                let sec = t.remaining.ceil().max(0.0) as i32;
                if sec > 0 && sec < 3 && sec < t.last_alarm_sec {
                    events.push(SimEvent::PlaySfx("AlarmBeepSound"));
                }
                t.last_alarm_sec = sec;
                if t.remaining <= 0.0 {
                    t.remaining = 0.0;
                    t.trigger_fade();
                    match ptype {
                        PieceType::Source => ready_sources = true,
                        PieceType::Portal => ready_portals.push((x, y)),
                        _ => {}
                    }
                }
            }
        }
        if ready_sources {
            self.start_source_fill(events);
        }
        for (px, py) in ready_portals {
            self.tiles[py][px].start_filling(Side::Center, fast_nozzle);
        }
    }

    pub(crate) fn tick_fill_passes(
        &mut self,
        dt: f32,
        events: &mut Vec<SimEvent>,
    ) -> Vec<(usize, usize, Side)> {
        let fast_nozzle = self.is_accelerated || self.current_speed >= 15.0;
        let mut completed_exits = Vec::new();
        for y in 0..self.level.rows {
            for x in 0..self.level.cols {
                let tile = &mut self.tiles[y][x];
                let divisor = if tile.is_freeze && self.current_speed < 120.0 {
                    8000.0
                } else {
                    1000.0
                };
                let rate = (self.current_speed / divisor) * 30.0;
                for pass in &mut tile.fills {
                    if pass.completed {
                        continue;
                    }
                    pass.elapsed = if fast_nozzle {
                        1.0
                    } else {
                        (pass.elapsed + dt).min(1.0)
                    };
                    let needs_slime = tile.piece_type == PieceType::Drain
                        || (tile.piece_type == PieceType::Portal && pass.entry != Side::Center);
                    if needs_slime {
                        if tile.slime_done {
                            pass.progress = 1.0;
                            pass.completed = true;
                            completed_exits.push((x, y, pass.exit));
                            continue;
                        }
                        let next_prog = pass.progress + rate * dt;
                        if next_prog >= 0.6 {
                            pass.progress = 0.6;
                            if tile.slime_anim.is_none() {
                                tile.slime_anim = Some(0.0);
                                tile.slime_playing = true;
                                events.push(SimEvent::PlaySfx("BlubThreeSound"));
                            }
                            continue;
                        }
                        pass.progress = next_prog;
                        continue;
                    }
                    pass.progress = (pass.progress + rate * dt).min(1.0);
                    let threshold =
                        if tile.piece_type == PieceType::Portal && pass.entry == Side::Center {
                            1.0
                        } else {
                            0.9
                        };
                    if pass.progress >= threshold {
                        pass.completed = true;
                        completed_exits.push((x, y, pass.exit));
                    }
                }
            }
        }
        completed_exits
    }

    pub(crate) fn handle_completed_exits(
        &mut self,
        completed_exits: Vec<(usize, usize, Side)>,
        events: &mut Vec<SimEvent>,
    ) {
        for (x, y, exit_side) in completed_exits {
            if self.finished {
                break;
            }
            let ptype = self.tiles[y][x].piece_type;
            let sec_color = self.tiles[y][x].secondary_color;

            if ptype == PieceType::Drain {
                self.pipes_filled += 1;
                let res = calculate_score(
                    self.moves,
                    self.pipes_filled,
                    self.elapsed_ms as u32,
                    self.level.par_moves,
                    self.level.par_pipes,
                    self.level.par_time_ms,
                    self.difficulty == Difficulty::Hard,
                );
                self.score = res.total_score;
                self.finished = true;
                self.won = Some(true);
                self.last_result = Some(res);
                events.push(SimEvent::PlaySfx("CheeringSound"));
                events.push(SimEvent::LevelWon(res));
                break;
            }

            if !self.swapping_tiles.is_empty() {
                self.pending_exits.push((x, y, exit_side));
                continue;
            }

            self.pipes_filled += 1;
            if ptype == PieceType::Portal && exit_side == Side::Center {
                if let Some((mx, my)) = find_matching_portal(
                    &self.tiles,
                    self.level.rows,
                    self.level.cols,
                    x,
                    y,
                    sec_color,
                ) {
                    self.score += 100;
                    events.push(SimEvent::PlayRandomBonusSfx);
                    events.push(SimEvent::ScoreBlurb(mx as f32 + 0.5, my as f32 + 0.5, 100));
                    if self.selected == Some((mx, my)) {
                        self.selected = None;
                    }
                    if self.is_accelerated {
                        self.tiles[my][mx].start_filling(Side::Center, true);
                    } else {
                        self.tiles[my][mx].timer = Some(FlowTimerState::new(5.0));
                    }
                } else {
                    self.trigger_loss(x, y, Side::Center, events);
                }
                continue;
            }

            let (dx, dy) = exit_side.delta();
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx < 0 || ny < 0 || nx >= self.level.cols as i32 || ny >= self.level.rows as i32 {
                self.trigger_loss(x, y, exit_side, events);
                continue;
            }
            let (ux, uy) = (nx as usize, ny as usize);
            if self.selected == Some((ux, uy)) {
                self.selected = None;
            }
            let fast_nozzle = self.is_accelerated || self.current_speed >= 15.0;
            if self.tiles[uy][ux]
                .start_filling(exit_side.opposite(), fast_nozzle)
                .is_some()
            {
                self.score += 100;
                events.push(SimEvent::PlayRandomBonusSfx);
                events.push(SimEvent::ScoreBlurb(ux as f32 + 0.5, uy as f32 + 0.5, 100));
                if self.tiles[uy][ux].is_gate {
                    self.trigger_magic_tiles(events);
                }
            } else {
                self.trigger_loss(x, y, exit_side, events);
            }
        }
    }

    fn trigger_magic_tiles(&mut self, events: &mut Vec<SimEvent>) {
        let mut rotated_any = false;
        for row in &mut self.tiles {
            for mt in row {
                if mt.piece_type == PieceType::Pipe && mt.is_magic && !mt.has_filled {
                    mt.rotate_cw_timed(1, 0.4);
                    rotated_any = true;
                }
            }
        }
        if rotated_any {
            events.push(SimEvent::PlaySfx("TwistPieceSound"));
        }
    }

    fn trigger_loss(&mut self, tx: usize, ty: usize, exit_side: Side, events: &mut Vec<SimEvent>) {
        self.finished = true;
        self.won = Some(false);
        events.push(SimEvent::PlaySfx("PourSound"));
        events.push(SimEvent::LeakErupt(
            tx,
            ty,
            exit_side,
            self.level.liquid_color,
        ));
        events.push(SimEvent::LevelLost);
    }
}
