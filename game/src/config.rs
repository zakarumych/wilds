use {nalgebra as na, wilds::camera::Camera};

#[derive(Clone, Debug, serde::Deserialize)]
pub struct Config {
    // #[serde(flatten)]
    pub engine: wilds::config::Config,

    // #[serde(flatten)]
    pub game: GameConfig,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct GameConfig {
    #[serde(default)]
    pub camera: CameraConfig,
}

#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(untagged)]
pub enum CameraConfig {
    Perspective {
        #[serde(default = "default_fovy")]
        fovy: f32,
        #[serde(default = "default_znear")]
        znear: f32,
        #[serde(default = "default_zfar")]
        zfar: f32,
    },
}

impl Default for CameraConfig {
    fn default() -> Self {
        CameraConfig::Perspective {
            fovy: default_fovy(),
            znear: default_znear(),
            zfar: default_zfar(),
        }
    }
}

impl CameraConfig {
    pub fn into_camera(self, aspect: f32) -> Camera {
        match self {
            CameraConfig::Perspective { fovy, znear, zfar } => {
                Camera::Perspective(na::Perspective3::new(
                    aspect, fovy, znear, zfar,
                ))
            }
        }
    }
}

fn default_fovy() -> f32 {
    std::f32::consts::PI / 3.0
}

fn default_znear() -> f32 {
    0.1
}

fn default_zfar() -> f32 {
    1000.
}
