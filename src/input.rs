use std::collections::HashMap;

use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, Event, KeyEvent, StartCause, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

pub struct InputHandler {
    key_states: Vec<bool>,
    key_press_callbacks: HashMap<KeyCode, Vec<Box<dyn Fn()>>>,
    key_state_callbacks: HashMap<KeyCode, Vec<Box<dyn Fn(ElementState)>>>,
    cursor_callbacks: Vec<Box<dyn Fn(PhysicalPosition<f64>)>>,
}

impl InputHandler {
    pub fn new() -> Self {
        Self {
            key_states: vec![false; 194],
            key_press_callbacks: HashMap::new(),
            key_state_callbacks: HashMap::new(),
            cursor_callbacks: vec![],
        }
    }

    pub fn register_key_pressed_callback(&mut self, key: KeyCode, callback: Box<dyn Fn()>) {
        self.key_press_callbacks
            .entry(key)
            .or_default()
            .push(callback);
    }

    pub fn register_key_state_callback(
        &mut self,
        key: KeyCode,
        callback: Box<dyn Fn(ElementState)>,
    ) {
        self.key_state_callbacks
            .entry(key)
            .or_default()
            .push(callback);
    }

    pub fn register_cursor_callback(&mut self, callback: Box<dyn Fn(PhysicalPosition<f64>)>) {
        self.cursor_callbacks.push(callback);
    }

    pub fn handle_event(&mut self, event: Event<()>) {
        match event {
            Event::NewEvents(StartCause::Poll) => self
                .key_press_callbacks
                .iter()
                .filter(|(&key, ..)| self.key_states[key as usize])
                .for_each(|(_, callbacks)| callbacks.iter().for_each(|callback| callback())),
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            physical_key: PhysicalKey::Code(key),
                            state,
                            repeat: false,
                            ..
                        },
                    ..
                } => {
                    self.key_states[key as usize] = state.is_pressed();
                    if let Some(callbacks) = self.key_state_callbacks.get(&key) {
                        callbacks.iter().for_each(|callback| callback(state));
                    }
                }
                WindowEvent::CursorMoved { position, .. }
                    if position.x != 0.0 || position.y != 0.0 =>
                {
                    self.cursor_callbacks
                        .iter()
                        .for_each(|callback| callback(position))
                }
                _ => (),
            },
            _ => (),
        }
    }
}
