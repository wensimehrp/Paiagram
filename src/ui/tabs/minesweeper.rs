use super::Tab;
use bevy::prelude::*;
use egui::{Rect, Ui, Vec2};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(PartialEq, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MinesweeperTab;

impl Tab for MinesweeperTab {
    const NAME: &'static str = "Minesweeper";
    fn main_display(&mut self, world: &mut bevy::ecs::world::World, ui: &mut Ui) {
        if let Err(e) = world.run_system_cached_with(show_minesweeper, ui) {
            bevy::log::error!("UI Error while displaying minesweeper page: {}", e)
        }
    }
    fn edit_display(&mut self, world: &mut bevy::ecs::world::World, ui: &mut Ui) {
        ui.heading("Minesweeper");
        ui.label("Tired of trains? Here's a minesweeper!");
        ui.add_space(10.0);
        let time = world.resource::<Time>().elapsed();
        let mut data = world.resource_mut::<MinesweeperData>();
        const SEGMENT_SPACING: f32 = 5.0;
        let width = (ui.available_width() - SEGMENT_SPACING - SEGMENT_SPACING) / 3.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = SEGMENT_SPACING;
            if ui
                .add_sized([width, 30.0], egui::Button::new("Easy"))
                .clicked()
            {
                data.map.initialize(MinesweeperDifficulty::Easy);
                data.started = true;
                data.game_over = false;
                data.won = false;
                data.start_time = time;
                data.elapsed = Duration::ZERO;
            }
            if ui
                .add_sized([width, 30.0], egui::Button::new("Medium"))
                .clicked()
            {
                data.map.initialize(MinesweeperDifficulty::Medium);
                data.started = true;
                data.game_over = false;
                data.won = false;
                data.start_time = time;
                data.elapsed = Duration::ZERO;
            }
            if ui
                .add_sized([width, 30.0], egui::Button::new("Hard"))
                .clicked()
            {
                data.map.initialize(MinesweeperDifficulty::Hard);
                data.started = true;
                data.game_over = false;
                data.won = false;
                data.start_time = time;
                data.elapsed = Duration::ZERO;
            }
        });
        ui.label(format!("Elapsed: {:.0}s", data.elapsed.as_secs_f32()));
        ui.ctx().request_repaint_after(Duration::from_millis(100));
        ui.heading("High scores");
        for (i, record) in data.record.iter().enumerate() {
            ui.label(format!("{}. {:.2}s", i + 1, record.as_secs_f32()));
        }
    }
}

const MINE_STR: &str = "💣";
const FLAG_STR: &str = "🚩";

enum MinesweeperDifficulty {
    Easy,
    Medium,
    Hard,
}

impl MinesweeperDifficulty {
    fn parameters(&self) -> (u8, u8, u8) {
        match self {
            MinesweeperDifficulty::Easy => (9, 9, 10),
            MinesweeperDifficulty::Medium => (16, 16, 40),
            MinesweeperDifficulty::Hard => (30, 16, 99),
        }
    }
}

#[derive(Resource, Default)]
pub struct MinesweeperData {
    map: MinesweeperMap,
    started: bool,
    game_over: bool,
    won: bool,
    record: Vec<Duration>,
    start_time: Duration,
    elapsed: Duration,
}

#[derive(Default)]
struct MinesweeperMap {
    width: u8,
    height: u8,
    mines: Vec<(u8, u8)>,
    revealed: Vec<(u8, u8)>,
    flagged: Vec<(u8, u8)>,
}

impl MinesweeperMap {
    fn initialize(&mut self, difficulty: MinesweeperDifficulty) -> bool {
        let (width, height, mines) = difficulty.parameters();
        self.width = width;
        self.height = height;
        self.mines.clear();
        self.revealed.clear();
        self.flagged.clear();
        self.generate_mines(mines)
    }
    fn generate_mines(&mut self, mine_count: u8) -> bool {
        // Simple random mine generation (not optimized)
        use rand::Rng;
        let mut rng = rand::rng();
        if self.width as u16 * self.height as u16 <= mine_count as u16 {
            return false;
        }
        while self.mines.len() < mine_count as usize {
            let x = rng.random_range(0..self.width);
            let y = rng.random_range(0..self.height);
            if !self.mines.contains(&(x, y)) {
                self.mines.push((x, y));
            }
        }
        true
    }
    fn nearby_mines(&self, x: u8, y: u8) -> u8 {
        let mut count = 0;
        for xi in x.saturating_sub(1)..=(x + 1).min(self.width - 1) {
            for yi in y.saturating_sub(1)..=(y + 1).min(self.height - 1) {
                if self.mines.contains(&(xi, yi)) {
                    count += 1;
                }
            }
        }
        count
    }
    fn reveal(&mut self, x: u8, y: u8) -> bool {
        if self.mines.contains(&(x, y)) {
            return true; // Boom
        }
        if self.revealed.contains(&(x, y)) || self.flagged.contains(&(x, y)) {
            return false;
        }

        self.revealed.push((x, y));

        // If this cell is empty, reveal all neighbors
        if self.nearby_mines(x, y) == 0 {
            for xi in x.saturating_sub(1)..=(x + 1).min(self.width - 1) {
                for yi in y.saturating_sub(1)..=(y + 1).min(self.height - 1) {
                    self.reveal(xi, yi);
                }
            }
        }
        false
    }
    fn flag(&mut self, x: u8, y: u8) {
        if self.revealed.contains(&(x, y)) {
            return;
        }
        let i = self.flagged.iter().position(|&p| p == (x, y));
        if let Some(i) = i {
            self.flagged.remove(i);
        } else {
            self.flagged.push((x, y));
        }
    }
}

fn show_minesweeper(InMut(ui): InMut<Ui>, mut data: ResMut<MinesweeperData>, time: Res<Time>) {
    if !data.started {
        ui.centered_and_justified(|ui| ui.heading("Start from assistance panel"));
        return;
    }

    if !data.game_over {
        data.elapsed = time.elapsed().saturating_sub(data.start_time);
    }

    fn show_mine(x: u8, y: u8, ui: &mut Ui, data: &mut MinesweeperData) {
        let is_revealed = data.map.revealed.contains(&(x, y));
        let is_flagged = data.map.flagged.contains(&(x, y));
        let is_mine = data.map.mines.contains(&(x, y));

        let mut text = egui::WidgetText::from(" ");
        if is_revealed {
            if is_mine {
                text = MINE_STR.into();
            } else {
                let count = data.map.nearby_mines(x, y);
                if count > 0 {
                    text = count.to_string().into();
                }
            }
        } else if is_flagged {
            text = FLAG_STR.into();
        }

        let button = ui.add_sized([25.0, 25.0], egui::Button::new(text).selected(is_revealed));

        if !data.game_over {
            if button.clicked() && !is_flagged {
                if data.map.reveal(x, y) {
                    data.game_over = true;
                    // Reveal all mines on game over
                    let mines = data.map.mines.clone();
                    data.map.revealed.extend_from_slice(&mines);
                    data.map.revealed.sort_unstable();
                    data.map.revealed.dedup();
                }
            }
            if button.secondary_clicked() {
                data.map.flag(x, y);
            }
        }
    }

    ui.spacing_mut().item_spacing = Vec2 { x: 2.0, y: 2.0 };
    ui.spacing_mut().interact_size = Vec2::ZERO;
    let h_space = (ui.available_width() - data.map.width as f32 * 30.0 + 5.0) / 2.0;
    let v_space = (ui.available_height() - data.map.height as f32 * 30.0 + 5.0) / 2.0;
    let max_rect = ui.max_rect();
    ui.add_space(v_space.max(0.0));
    ui.horizontal(|ui| {
        ui.add_space(h_space.max(0.0));
        egui::Grid::new("minesweeper_grid")
            .spacing(Vec2 { x: 5.0, y: 5.0 })
            .show(ui, |ui| {
                for y in 0..data.map.height {
                    for x in 0..data.map.width {
                        show_mine(x, y, ui, &mut data);
                    }
                    ui.end_row();
                }
            });
        let total_cells = (data.map.width as usize) * (data.map.height as usize);
        let revealed_cells = data.map.revealed.len();
        let mine_count = data.map.mines.len();
        if !data.won && revealed_cells + mine_count == total_cells {
            data.won = true;
            data.game_over = true;
            let elapsed = data.elapsed;
            data.record.push(elapsed);
            data.record.sort();
        }
    });
    if data.won {
        ui.place(
            Rect::from_pos(max_rect.center()).expand2(Vec2 { x: 80.0, y: 20.0 }),
            |ui: &mut Ui| {
                ui.painter().rect(
                    ui.max_rect(),
                    5.0,
                    ui.visuals().widgets.active.bg_fill,
                    ui.visuals().widgets.active.bg_stroke,
                    egui::StrokeKind::Middle,
                );
                ui.centered_and_justified(|ui: &mut Ui| ui.heading("You won!"))
                    .response
            },
        );
    }
}
