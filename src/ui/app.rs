use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use std::time::Duration;

use crate::config::Tunable;
use crate::tuner::run_tuning;
use crate::ui::input::{adjust_field, edit_field};
use crate::ui::types::{Field, Focus};

pub struct App {
    pub state: Tunable,
    pub selected: usize,
    pub focus: Focus,
    pub log_scroll: usize,
    pub fields: Vec<Field>,
    pub logs: Vec<String>,
    pub tuning_handle: Option<std::thread::JoinHandle<()>>,
    pub log_tx: std::sync::mpsc::Sender<String>,
    pub log_rx: std::sync::mpsc::Receiver<String>,
}

impl App {
    pub fn new() -> Self {
        let (log_tx, log_rx): (
            std::sync::mpsc::Sender<String>,
            std::sync::mpsc::Receiver<String>,
        ) = std::sync::mpsc::channel();

        Self {
            state: Tunable::default(),
            selected: 0,
            focus: Focus::Params,
            log_scroll: 0,
            fields: vec![
                Field::TestFileUrl,
                Field::LatencyUrl,
                Field::MinUp,
                Field::MaxUp,
                Field::MinDown,
                Field::MaxDown,
                Field::TargetAccuracy,
            ],
            logs: Vec::new(),
            tuning_handle: None,
            log_tx,
            log_rx,
        }
    }

    pub fn handle_key_event(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Char('q') => return false, // 退出
            KeyCode::Tab => {
                self.focus = if self.focus == Focus::Params {
                    Focus::Logs
                } else {
                    Focus::Params
                };
            }
            KeyCode::Up => match self.focus {
                Focus::Params => {
                    if self.selected > 0 {
                        self.selected -= 1;
                    }
                }
                Focus::Logs => {
                    self.log_scroll = self.log_scroll.saturating_add(1);
                }
            },
            KeyCode::Down => match self.focus {
                Focus::Params => {
                    if self.selected + 1 < self.fields.len() {
                        self.selected += 1;
                    }
                }
                Focus::Logs => {
                    self.log_scroll = self.log_scroll.saturating_sub(1);
                }
            },
            KeyCode::PageUp => {
                if self.focus == Focus::Logs {
                    self.log_scroll = self.log_scroll.saturating_add(5);
                }
            }
            KeyCode::PageDown => {
                if self.focus == Focus::Logs {
                    self.log_scroll = self.log_scroll.saturating_sub(5);
                }
            }
            KeyCode::Left => {
                if self.focus == Focus::Params {
                    adjust_field(&mut self.state, &self.fields[self.selected], -1)
                }
            }
            KeyCode::Right => {
                if self.focus == Focus::Params {
                    adjust_field(&mut self.state, &self.fields[self.selected], 1)
                }
            }
            KeyCode::Char('e') => {
                if self.focus == Focus::Params {
                    let _ = edit_field(&mut self.state, &self.fields[self.selected]);
                }
            }
            KeyCode::Char('s') => {
                if self.tuning_handle.is_none() {
                    let cfg = self.state.clone();
                    let tx = self.log_tx.clone();
                    self.logs.clear();
                    self.log_scroll = 0;
                    self.tuning_handle = Some(std::thread::spawn(move || run_tuning(cfg, tx)));
                }
            }
            _ => {}
        }
        true
    }

    pub fn update_logs(&mut self) {
        while let Ok(line) = self.log_rx.try_recv() {
            self.logs.push(line);
            if self.logs.len() > 200 {
                self.logs.drain(0..self.logs.len() - 200);
            }
            self.log_scroll = 0;
        }
    }

    pub fn check_tuning_complete(&mut self) {
        if let Some(handle) = self.tuning_handle.as_ref() {
            if handle.is_finished() {
                let _ = self.tuning_handle.take().unwrap().join();
            }
        }
    }

    pub fn run_event_loop(&mut self) -> anyhow::Result<bool> {
        self.update_logs();

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press {
                    return Ok(self.handle_key_event(k.code));
                }
            }
        }

        self.check_tuning_complete();
        Ok(true)
    }
}
