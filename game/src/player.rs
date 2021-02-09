use {
    hecs::Entity,
    std::collections::HashMap,
    wilds::engine::{System, SystemContext},
    winit::{
        dpi::PhysicalSize,
        event::{
            ElementState, Event, KeyboardInput, MouseButton, MouseScrollDelta,
            VirtualKeyCode, WindowEvent,
        },
        window::{Window, WindowId},
    },
};

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum InputEvent {
    KeyboardInput {
        code: VirtualKeyCode,
        state: ElementState,
    },
    MouseInput {
        button: MouseButton,
        state: ElementState,
    },
    MouseWheelUp,
    MouseWheelDown,
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum Action {
    MoveToCursor,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ActionMap {
    map: HashMap<InputEvent, Action>,
}

impl ActionMap {
    pub fn get(&self, input: &InputEvent) -> Option<Action> {
        self.map.get(input).copied()
    }
}

impl Default for ActionMap {
    fn default() -> Self {
        let mut map = HashMap::new();

        map.insert(
            InputEvent::MouseInput {
                button: MouseButton::Left,
                state: ElementState::Pressed,
            },
            Action::MoveToCursor,
        );

        ActionMap { map }
    }
}

/// Player controller.
/// Takes input events
/// and controls player's character.
pub struct Player {
    cursor_pos: [f64; 2],
    window_size: PhysicalSize<u32>,
    window_id: WindowId,
    action_map: ActionMap,
    faction: usize,
}

impl Player {
    pub fn new(window: &Window, faction: usize) -> Self {
        Self::with_action_map(window, faction, ActionMap::default())
    }

    pub fn with_action_map(
        window: &Window,
        faction: usize,
        action_map: ActionMap,
    ) -> Self {
        Player {
            cursor_pos: [0.5; 2],
            action_map,
            faction,
            window_size: window.inner_size(),
            window_id: window.id(),
        }
    }

    fn translate_event(&mut self, event: &Event<'_, ()>) -> Option<Action> {
        match event {
            Event::WindowEvent { event, window_id }
                if *window_id == self.window_id =>
            {
                match event {
                    &WindowEvent::Resized(size) => {
                        self.window_size = size;
                        None
                    }
                    &WindowEvent::CursorMoved { position, .. } => {
                        self.cursor_pos = [
                            (position.x / self.window_size.width as f64),
                            (position.y / self.window_size.height as f64),
                        ];
                        None
                    }
                    &WindowEvent::MouseWheel {
                        delta: MouseScrollDelta::LineDelta(_, y),
                        ..
                    } => {
                        if y > 0.1 {
                            Some(InputEvent::MouseWheelUp)
                        } else if y < -0.1 {
                            Some(InputEvent::MouseWheelDown)
                        } else {
                            None
                        }
                    }
                    &WindowEvent::MouseInput { button, state, .. } => {
                        Some(InputEvent::MouseInput { button, state })
                    }
                    &WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(code),
                                state,
                                ..
                            },
                        ..
                    } => Some(InputEvent::KeyboardInput { code, state }),
                    _ => None,
                }
            }
            _ => None,
        }
        .and_then(|event| self.action_map.get(&event))
    }
}

impl System for Player {
    fn name(&self) -> &str {
        "Player"
    }
    fn run(&mut self, ctx: SystemContext<'_>) {
        for event in ctx.input.read() {
            if let Some(action) = self.translate_event(&*event) {
                event.consume();

                match action {
                    Action::MoveToCursor => {
                        tracing::error!(
                            "MoveToCursor {{{:?}}}",
                            self.cursor_pos
                        );
                    }
                }
            }
        }
    }
}
