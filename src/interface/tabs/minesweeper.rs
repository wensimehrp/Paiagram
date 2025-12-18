//! Minesweeper
//! This is the Minesweeper game in my app
use std::time::Duration;

use arrayvec::ArrayVec;
use bevy::{
    ecs::system::{InMut, Local, Res},
    time::Time,
};
use egui::Ui;
const MINE_STR: &str = "💣";
const FLAG_STR: &str = "🚩";

struct MinesweeperMap {
    width: u8,
    height: u8,
    mines: ArrayVec<(u8, u8), 256>,
    revealed: ArrayVec<(u8, u8), 256>,
    flagged: ArrayVec<(u8, u8), 256>,
}

pub fn show_minesweeper(
    InMut(ui): InMut<Ui>,
    mut scores: Local<Vec<Duration>>,
    time: Res<Time>,
    mut map: Local<Option<MinesweeperMap>>,
) {
    ui.heading("Minesweeper");
}
