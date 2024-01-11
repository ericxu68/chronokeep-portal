use std::{sync::{Arc, Mutex, Condvar}, time::{Duration, Instant}};

use crate::util;

pub struct Sounds {
    control: super::Control,
    sound_notifier: Arc<Condvar>,
    keepalive: Arc<Mutex<bool>>,
    mtx: Arc<Mutex<bool>>
}

impl Sounds {
    pub fn new(
        control: super::Control,
        sound_notifier: Arc<Condvar>,
        keepalive: Arc<Mutex<bool>>
    ) -> Sounds {
        Sounds {
            control,
            sound_notifier,
            keepalive,
            mtx: Arc::new(Mutex::new(true))
        }
    }

    pub fn run(&mut self) {
        let mut last_sound = Instant::now();
        loop {
            if let Ok(ka) = self.keepalive.try_lock() {
                match *ka {
                    false => {
                        break
                    },
                    true => {},
                }
            }
            let notifier = self.mtx.lock().unwrap();
            if let Ok(_) = self.sound_notifier.wait(notifier) {
                if self.control.play_sound == true && last_sound.elapsed() >= Duration::from_millis(500) {
                    util::play_sound(self.control.volume);
                    last_sound = Instant::now();
                }
            } else {
                println!("Error waiting to play a sound.");
            }
        }
    }
}