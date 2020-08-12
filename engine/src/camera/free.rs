use {
    super::Camera,
    crate::engine::{Resources, System},
    std::f32::consts::{FRAC_PI_2, PI},
    ultraviolet::{Isometry3, Rotor3, Vec3},
    winit::event::{DeviceEvent, ElementState, Event, VirtualKeyCode},
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
    direction: Direction,
    pitch_factor: f32,
    yaw_factor: f32,
    speed: f32,
}

impl FreeCameraSystem {
    pub fn new() -> Self {
        FreeCameraSystem {
            pitch: 0.0,
            yaw: 0.0,
            direction: Direction::empty(),
            pitch_factor: 1.0,
            yaw_factor: 1.0,
            speed: 1.0,
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
    fn run(&mut self, resources: Resources<'_>) {
        let delta = resources.clocks.delta.as_secs_f32();
        let mut query = resources
            .world
            .query::<&mut Isometry3>()
            .with::<Camera>()
            .with::<FreeCamera>();

        if let Some((_, iso)) = query.iter().next() {
            for event in resources.input.read() {
                match event {
                    Event::DeviceEvent { event, .. } => match event {
                        &DeviceEvent::MouseMotion { delta: (x, y) } => {
                            // let x = Rotor3::from_rotation_xz(
                            //     x as f32 * delta,
                            // );
                            // let y = Rotor3::from_rotation_yz(
                            //     -y as f32 * delta,
                            // );
                            // iso.rotation = x * iso.rotation * y;

                            self.pitch -= y as f32 * delta * self.pitch_factor;
                            self.yaw += x as f32 * delta * self.yaw_factor;

                            self.pitch =
                                self.pitch.min(FRAC_PI_2).max(-FRAC_PI_2);

                            if self.yaw < -PI {
                                self.yaw -= (self.yaw / TAU).floor() * TAU;
                            }

                            if self.yaw > PI {
                                self.yaw -= (self.yaw / TAU).ceil() * TAU;
                            }

                            iso.rotation = Rotor3::from_euler_angles(
                                0.0, self.pitch, self.yaw,
                            )
                        }
                        DeviceEvent::Key(input) => {
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

            let mut moving = Vec3::new(0.0, 0.0, 0.0);
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

            iso.prepend_translation(moving);
        }
    }
}
