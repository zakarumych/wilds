use {
    super::Camera,
    crate::{
        engine::{System, SystemContext},
        scene::Global3,
    },
    hecs::Entity,
    nalgebra as na,
    std::f32::consts::{FRAC_PI_2, PI, TAU},
    winit::event::{DeviceEvent, ElementState, Event, VirtualKeyCode},
};

#[derive(Clone, Copy)]
/// Following camera marker component.
pub struct FollowingCamera {
    pub follows: Entity,
}

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
pub struct FollowingCameraSystem {
    pitch: f32,
    roll: f32,
    distance: f32,
    pitch_factor: f32,
    roll_factor: f32,
    speed: f32,
    direction: Direction,
}

impl FollowingCameraSystem {
    pub fn new() -> Self {
        FollowingCameraSystem {
            pitch: FRAC_PI_2 / 2.0,
            roll: 0.0,
            distance: 5.0,
            pitch_factor: 1.0,
            roll_factor: 1.0,
            speed: 1.0,
            direction: Direction::empty(),
        }
    }

    pub fn with_factor(mut self, pitch: f32, roll: f32) -> Self {
        self.pitch_factor = pitch;
        self.roll_factor = roll;
        self
    }

    pub fn with_speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }
}

impl System for FollowingCameraSystem {
    fn name(&self) -> &str {
        "Following camera"
    }

    fn run(&mut self, ctx: SystemContext<'_>) {
        let world = ctx.world;
        let delta = ctx.clocks.delta.as_secs_f32();

        for event in ctx.input.read() {
            match &*event {
                Event::DeviceEvent { event, .. } => match event {
                    &DeviceEvent::MouseMotion { delta: (x, y) } => {
                        self.roll -= y as f32 * self.pitch_factor;
                        self.pitch -= x as f32 * self.roll_factor;

                        self.roll = self.roll.min(FRAC_PI_2).max(-FRAC_PI_2);

                        if self.pitch < -PI {
                            self.pitch -= (self.pitch / TAU).floor() * TAU;
                        }

                        if self.pitch > PI {
                            self.pitch -= (self.pitch / TAU).ceil() * TAU;
                        }
                    }
                    DeviceEvent::Key(input) => {
                        let flag = match input.virtual_keycode {
                            Some(VirtualKeyCode::W) => Direction::FORWARD,
                            Some(VirtualKeyCode::S) => Direction::BACKWARD,
                            Some(VirtualKeyCode::A) => Direction::LEFT,
                            Some(VirtualKeyCode::D) => Direction::RIGHT,
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

        if self.direction.contains(Direction::FORWARD) {
            self.distance -= self.speed * delta;
        }
        if self.direction.contains(Direction::BACKWARD) {
            self.distance += self.speed * delta;
        }

        if self.direction.contains(Direction::LEFT) {
            self.pitch -= delta * self.pitch_factor;
        }
        if self.direction.contains(Direction::RIGHT) {
            self.pitch += delta * self.pitch_factor;
        }

        let found = world
            .query::<&FollowingCamera>()
            .with::<Camera>()
            .with::<Global3>()
            .iter()
            .next()
            .map(|(e, f)| (e, *f));

        if let Some((camera, following)) = found {
            let mut global = world
                .get::<Global3>(following.follows)
                .ok()
                .as_deref()
                .cloned()
                .unwrap_or_else(Global3::identity);

            let rotation = na::UnitQuaternion::from_euler_angles(
                self.roll,
                -self.pitch,
                0.0,
            );

            let translation = rotation
                .transform_vector(&na::Vector3::new(0.0, 0.0, self.distance))
                .into();

            global.iso *= na::Isometry3 {
                rotation,
                translation,
            };

            *world.get_mut::<Global3>(camera).unwrap() = global;
        }
    }
}
