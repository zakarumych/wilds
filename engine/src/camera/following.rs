use {
    super::Camera,
    crate::engine::{System, SystemContext},
    hecs::Entity,
    std::f32::consts::FRAC_PI_2,
    ultraviolet::{Isometry3, Rotor3, Vec3},
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
    yaw: f32,
    distance: f32,
    pitch_factor: f32,
    yaw_factor: f32,
    speed: f32,
    direction: Direction,
}

impl FollowingCameraSystem {
    pub fn new() -> Self {
        FollowingCameraSystem {
            pitch: FRAC_PI_2 / 2.0,
            yaw: FRAC_PI_2 / 2.0,
            distance: 5.0,
            pitch_factor: 1.0,
            yaw_factor: 1.0,
            speed: 1.0,
            direction: Direction::empty(),
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

impl System for FollowingCameraSystem {
    fn run(&mut self, ctx: SystemContext<'_>) {
        let world = ctx.world;
        let delta = ctx.clocks.delta.as_secs_f32();

        for event in ctx.input.read() {
            match event {
                Event::DeviceEvent { event, .. } => match event {
                    // &DeviceEvent::MouseMotion { delta: (x, y) } => {
                    //     self.pitch += y as f32 * delta * self.pitch_factor;
                    //     self.yaw += x as f32 * delta * self.yaw_factor;

                    //     self.pitch =
                    // self.pitch.min(FRAC_PI_2).max(-FRAC_PI_2);

                    //     if self.yaw < -PI {
                    //         self.yaw -= (self.yaw / TAU).floor() * TAU;
                    //     }

                    //     if self.yaw > PI {
                    //         self.yaw -= (self.yaw / TAU).ceil() * TAU;
                    //     }
                    // }
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
            self.yaw -= delta * self.yaw_factor;
        }
        if self.direction.contains(Direction::RIGHT) {
            self.yaw += delta * self.yaw_factor;
        }

        let found = world
            .query::<&FollowingCamera>()
            .with::<Camera>()
            .with::<Isometry3>()
            .iter()
            .next()
            .map(|(e, f)| (e, *f));

        if let Some((camera, following)) = found {
            let mut iso = world
                .get::<Isometry3>(following.follows)
                .ok()
                .as_deref()
                .cloned()
                .unwrap_or_default();

            let rotation =
                Rotor3::from_euler_angles(0.0, -self.pitch, self.yaw);
            let translation =
                (Vec3::unit_z() * self.distance).rotated_by(rotation);
            iso.prepend_isometry(Isometry3 {
                rotation,
                translation,
            });

            *world.get_mut::<Isometry3>(camera).unwrap() = iso;
        }
    }
}
