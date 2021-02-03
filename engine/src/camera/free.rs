use {
    super::Camera,
    crate::{
        engine::{System, SystemContext},
        scene::Global3,
    },
    nalgebra as na,
    std::f32::consts::{FRAC_PI_2, PI},
    winit::event::{
        DeviceEvent, ElementState, Event, KeyboardInput, VirtualKeyCode,
    },
};

const TAU: f32 = 6.28318530717958647692528676655900577f32;

/// Free camera marker component.
pub struct FreeCamera;

bitflags::bitflags! {
    pub struct Direction: u8 {
        const FORWARD = 0b000001;
        const BACKWARD = 0b000010;
        const LEFT = 0b000100;
        const RIGHT = 0b001000;
        const UP = 0b010000;
        const DOWN = 0b100000;
    }
}

/// System to fly camera freely.
pub struct FreeCameraSystem {
    pitch: f32,
    yaw: f32,
    roll: f32,
    direction: Direction,
    pitch_factor: f32,
    yaw_factor: f32,
    speed: f32,
    enabled: bool,
}

impl FreeCameraSystem {
    pub fn new() -> Self {
        FreeCameraSystem {
            pitch: 0.0,
            yaw: 0.0,
            roll: 0.0,
            direction: Direction::empty(),
            pitch_factor: 1.0,
            yaw_factor: 1.0,
            speed: 1.0,
            enabled: true,
        }
    }

    pub fn with_factor(mut self, pitch: f32, yaw: f32) -> Self {
        self.pitch_factor = pitch;
        self.yaw_factor = yaw;
        self
    }

    pub fn with_speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }
}

impl System for FreeCameraSystem {
    fn run(&mut self, ctx: SystemContext<'_>) {
        let delta = ctx.clocks.delta.as_secs_f32();
        let mut query = ctx
            .world
            .query::<&mut Global3>()
            .with::<Camera>()
            .with::<FreeCamera>();

        if let Some((_, global)) = query.iter().next() {
            for event in ctx.input.read() {
                match &*event {
                    Event::DeviceEvent { event, .. } => match event {
                        DeviceEvent::Key(KeyboardInput {
                            virtual_keycode: Some(VirtualKeyCode::Z),
                            state: ElementState::Released,
                            ..
                        }) => {
                            self.enabled = !self.enabled;
                        }
                        &DeviceEvent::MouseMotion { delta: (x, y) }
                            if self.enabled =>
                        {
                            self.roll -= y as f32 * self.pitch_factor;
                            self.pitch -= x as f32 * self.yaw_factor;

                            self.roll =
                                self.roll.min(FRAC_PI_2).max(-FRAC_PI_2);

                            if self.pitch < -PI {
                                self.pitch -= (self.pitch / TAU).floor() * TAU;
                            }

                            if self.pitch > PI {
                                self.pitch -= (self.pitch / TAU).ceil() * TAU;
                            }

                            global.iso.rotation =
                                na::UnitQuaternion::from_euler_angles(
                                    self.roll, self.pitch, self.yaw,
                                )
                        }
                        DeviceEvent::Key(input) if self.enabled => {
                            let flag = match input.virtual_keycode {
                                Some(VirtualKeyCode::W) => Direction::FORWARD,
                                Some(VirtualKeyCode::S) => Direction::BACKWARD,
                                Some(VirtualKeyCode::A) => Direction::LEFT,
                                Some(VirtualKeyCode::D) => Direction::RIGHT,
                                Some(VirtualKeyCode::Space) => Direction::UP,
                                Some(VirtualKeyCode::LControl) => {
                                    Direction::DOWN
                                }
                                _ => continue,
                            };

                            match input.state {
                                ElementState::Pressed => {
                                    self.direction.insert(flag)
                                }
                                ElementState::Released => {
                                    self.direction.remove(flag)
                                }
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }

            let mut moving = na::Vector3::new(0.0, 0.0, 0.0);
            if self.direction.contains(Direction::FORWARD) {
                moving[2] -= 1.0;
            }
            if self.direction.contains(Direction::BACKWARD) {
                moving[2] += 1.0;
            }
            if self.direction.contains(Direction::LEFT) {
                moving[0] -= 1.0;
            }
            if self.direction.contains(Direction::RIGHT) {
                moving[0] += 1.0;
            }
            if self.direction.contains(Direction::UP) {
                moving[1] += 1.0;
            }
            if self.direction.contains(Direction::DOWN) {
                moving[1] -= 1.0;
            }

            moving *= self.speed * delta;

            global.iso *= na::Translation::from(moving);
        }
    }
}
