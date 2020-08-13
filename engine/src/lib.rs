// #![deny(unused_imports)]
// #![deny(unused)]

pub mod assets;
pub mod broker;
pub mod camera;
pub mod clocks;
pub mod config;
pub mod engine;
pub mod fps_counter;
pub mod light;
pub mod physics;
pub mod renderer;
pub mod util;

use {
    stats_alloc::{StatsAlloc, INSTRUMENTED_SYSTEM},
    std::alloc::System,
};

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

pub mod alloc {
    pub struct Region {
        inner: stats_alloc::Region<'static, std::alloc::System>,
    }

    impl Region {
        pub fn new() -> Self {
            Region {
                inner: stats_alloc::Region::new(crate::GLOBAL),
            }
        }

        pub fn initial(&self) -> stats_alloc::Stats {
            self.inner.initial()
        }

        pub fn change(&self) -> stats_alloc::Stats {
            self.inner.change()
        }

        pub fn change_and_reset(&mut self) -> stats_alloc::Stats {
            self.inner.change_and_reset()
        }

        pub fn reset(&mut self) {
            self.inner.reset()
        }
    }
}
