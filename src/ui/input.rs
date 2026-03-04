use anyhow::Result;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::io::{self, Write};

use crate::config::Tunable;
use crate::ui::types::Field;

pub fn prompt_input(label: &str, current: &str) -> Result<String> {
    disable_raw_mode()?;
    print!("{label} (当前: {current}) 新值: ");
    io::stdout().flush()?;
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    enable_raw_mode()?;
    let trimmed = buf.trim();
    if trimmed.is_empty() {
        Ok(current.to_string())
    } else {
        Ok(trimmed.to_string())
    }
}

pub fn prompt_number(label: &str, current: u32) -> Result<u32> {
    disable_raw_mode()?;
    print!("{label} (当前: {current}) 新值: ");
    io::stdout().flush()?;
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    enable_raw_mode()?;
    let trimmed = buf.trim();
    if trimmed.is_empty() {
        Ok(current)
    } else {
        Ok(trimmed.parse::<u32>()?)
    }
}

pub fn edit_field(state: &mut Tunable, field: &Field) -> Result<()> {
    match field {
        Field::TestFileUrl => {
            state.test_file_url = prompt_input("TEST_FILE_URL", &state.test_file_url)?;
        }
        Field::LatencyUrl => {
            state.latency_url = prompt_input("LATENCY_URL", &state.latency_url)?;
        }
        Field::MinUp => state.min_up = prompt_number("MIN_UP", state.min_up)?,
        Field::MaxUp => state.max_up = prompt_number("MAX_UP", state.max_up)?,
        Field::MinDown => state.min_down = prompt_number("MIN_DOWN", state.min_down)?,
        Field::MaxDown => state.max_down = prompt_number("MAX_DOWN", state.max_down)?,
        Field::TargetAccuracy => {
            state.target_accuracy = prompt_number("TARGET_ACCURACY", state.target_accuracy)?
        }
    }
    Ok(())
}

pub fn adjust_field(state: &mut Tunable, field: &Field, delta: i32) {
    let step = 10;
    match field {
        Field::TestFileUrl | Field::LatencyUrl => {}
        Field::MinUp => state.min_up = ((state.min_up as i32) + delta * step).max(1) as u32,
        Field::MaxUp => state.max_up = ((state.max_up as i32) + delta * step).max(1) as u32,
        Field::MinDown => state.min_down = ((state.min_down as i32) + delta * step).max(1) as u32,
        Field::MaxDown => state.max_down = ((state.max_down as i32) + delta * step).max(1) as u32,
        Field::TargetAccuracy => {
            state.target_accuracy = ((state.target_accuracy as i32) + delta).max(1) as u32
        }
    }
}
