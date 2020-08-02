use std::{collections::VecDeque, convert::TryFrom as _, time::Duration};

pub struct FpsCounter {
    samples: VecDeque<u32>,
    sum: u32,
    window: u32,
}

impl FpsCounter {
    pub fn new(window: Duration) -> Self {
        let window = window.as_micros();
        let window = u32::try_from(window).expect("Too large window");
        FpsCounter {
            samples: VecDeque::new(),
            sum: 0,
            window,
        }
    }

    pub fn add_sample(&mut self, sample: Duration) {
        loop {
            match u32::try_from(sample.as_micros()) {
                Ok(sample) if sample < self.window => {
                    match self.sum.checked_add(sample) {
                        Some(sum) if sum <= self.window => {
                            self.sum = sum;
                            self.samples.push_back(sample);
                            break;
                        }
                        _ => match self.samples.pop_front() {
                            Some(sample) => {
                                match self.sum.checked_sub(sample) {
                                    Some(left) => self.sum = left,
                                    None => panic!(
                                        "Sum field is smaller than samples sum"
                                    ),
                                }
                            }
                            None => {
                                debug_assert_eq!(self.sum, 0);
                                unreachable!();
                            }
                        },
                    }
                }
                Ok(sample) => {
                    self.samples.clear();
                    self.samples.push_back(sample);
                    self.sum = sample;
                    break;
                }
                Err(_) => {
                    self.samples.clear();
                    self.samples.push_back(!0);
                    self.sum = !0;
                    break;
                }
            }
        }
    }

    pub fn average(&self) -> Duration {
        let micros = match u32::try_from(self.samples.len()) {
            Ok(0) => {
                debug_assert_eq!(self.sum, 0);
                0
            }
            Ok(num) => self.sum / num,
            Err(_) => 0,
        };
        Duration::from_micros(micros.into())
    }
}
