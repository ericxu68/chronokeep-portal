use std::{net::TcpStream, sync::{Arc, Mutex}, thread::{self, JoinHandle}, time::Duration};

use crate::{control::{self, socket::{self, MAX_CONNECTED}, sound::SoundNotifier}, database::sqlite, processor::{self, SightingsProcessor}};

pub const WAITING_PERIOD_SECONDS: u64 = 1;

pub struct Reconnector {
    readers: Arc<Mutex<Vec<super::Reader>>>,
    joiners: Arc<Mutex<Vec<JoinHandle<()>>>>,
    control_sockets: Arc<Mutex<[Option<TcpStream>;MAX_CONNECTED + 1]>>,
    read_repeaters: Arc<Mutex<[bool;MAX_CONNECTED]>>,
    sight_processor: Arc<processor::SightingsProcessor>,
    control: Arc<Mutex<control::Control>>,
    sqlite: Arc<Mutex<sqlite::SQLite>>,
    read_saver: Arc<processor::ReadSaver>,
    sound: Arc<SoundNotifier>,
    id: i64,
    count: i32
}


impl Reconnector {
    pub(crate) fn new_internal(
        readers: Arc<Mutex<Vec<super::Reader>>>,
        joiners: Arc<Mutex<Vec<JoinHandle<()>>>>,
        control: Arc<Mutex<control::Control>>,
        sqlite: Arc<Mutex<sqlite::SQLite>>,
        read_saver: Arc<processor::ReadSaver>,
        sound: Arc<SoundNotifier>,
        id: i64,
        count: i32
    ) -> Reconnector {
        let sight_processor = SightingsProcessor::new(
            Arc::new(Mutex::new(Default::default())),
            Arc::new(Mutex::new(Default::default())),
            sqlite.clone(),
            Arc::new(Mutex::new(false))
        );
        Reconnector {
            readers,
            joiners,
            control_sockets: Arc::new(Mutex::new(Default::default())),
            read_repeaters: Arc::new(Mutex::new(Default::default())),
            sight_processor: Arc::new(sight_processor),
            control,
            sqlite,
            read_saver,
            sound,
            id,
            count
        }
    }

    pub fn new(
        readers: Arc<Mutex<Vec<super::Reader>>>,
        joiners: Arc<Mutex<Vec<JoinHandle<()>>>>,
        control_sockets: Arc<Mutex<[Option<TcpStream>;MAX_CONNECTED + 1]>>,
        read_repeaters: Arc<Mutex<[bool;MAX_CONNECTED]>>,
        sight_processor: Arc<processor::SightingsProcessor>,
        control: Arc<Mutex<control::Control>>,
        sqlite: Arc<Mutex<sqlite::SQLite>>,
        read_saver: Arc<processor::ReadSaver>,
        sound: Arc<SoundNotifier>,
        id: i64,
        count: i32
    ) -> Reconnector {
        Reconnector {
            readers,
            joiners,
            control_sockets,
            read_repeaters,
            sight_processor,
            control,
            sqlite,
            read_saver,
            sound,
            id,
            count
        }
    }

    pub fn run(self) {
        // Try to connect at most 5 times.
        if self.count > 5 {
            return;
        }
        println!("Attempting to reconnect to reader. Attempt {0}.", self.count);
        if let Ok(mut readers) = self.readers.lock() {
            match readers.iter().position(|x| x.id() == self.id) {
                Some(ix) => {
                    let mut old_reader = readers.remove(ix);
                    println!("Reconnecting to reader {}.", old_reader.nickname());
                    old_reader.set_control_sockets(self.control_sockets.clone());
                    old_reader.set_read_repeaters(self.read_repeaters.clone());
                    old_reader.set_sight_processor(self.sight_processor.clone());
                    let reconnector = Reconnector::new(
                        self.readers.clone(),
                        self.joiners.clone(),
                        self.control_sockets.clone(),
                        self.read_repeaters.clone(),
                        self.sight_processor.clone(),
                        self.control.clone(),
                        self.sqlite.clone(),
                        self.read_saver.clone(),
                        self.sound.clone(),
                        self.id,
                        1
                    );
                    match old_reader.connect(&self.sqlite.clone(), &self.control.clone(), &self.read_saver.clone(), self.sound.clone(), Some(Arc::new(reconnector))) {
                        Ok(j) => {
                            if let Ok(mut join) = self.joiners.lock() {
                                join.push(j);
                            }
                            println!("Initializing reader.");
                            let mut count = 0;
                            loop {
                                count += 1;
                                if count > 5 {
                                    break;
                                }
                                match old_reader.initialize() {
                                    Ok(_) => {
                                        break;
                                    },
                                    Err(e) => {
                                        println!("Error initializing reader: {e}");
                                    }
                                }
                                // wait for a few seconds before retrying
                                thread::sleep(Duration::from_secs(WAITING_PERIOD_SECONDS));
                            }
                            if old_reader.is_reading() != Some(true) {
                                match old_reader.disconnect() {
                                    Ok(_) => {},
                                    Err(_) => {
                                        println!("error attempting to disconnect from reader before reconnect attempt")
                                    },
                                }
                                readers.push(old_reader);
                                thread::sleep(Duration::from_secs(WAITING_PERIOD_SECONDS));
                                let new_reconnector = Reconnector::new(
                                    self.readers.clone(),
                                    self.joiners.clone(),
                                    self.control_sockets.clone(),
                                    self.read_repeaters.clone(),
                                    self.sight_processor.clone(),
                                    self.control.clone(),
                                    self.sqlite.clone(),
                                    self.read_saver.clone(),
                                    self.sound.clone(),
                                    self.id,
                                    self.count + 1
                                );
                                new_reconnector.run();
                                // return so only on success will the control sockets be notified of changes
                                return;
                            }
                            readers.push(old_reader);
                        }
                        Err(e) => {
                            println!("Error connecting to reader: {e}");
                            // wait for a few seconds before retrying
                            readers.push(old_reader);
                            thread::sleep(Duration::from_secs(WAITING_PERIOD_SECONDS));
                            let new_reconnector = Reconnector::new(
                                self.readers.clone(),
                                self.joiners.clone(),
                                self.control_sockets.clone(),
                                self.read_repeaters.clone(),
                                self.sight_processor.clone(),
                                self.control.clone(),
                                self.sqlite.clone(),
                                self.read_saver.clone(),
                                self.sound.clone(),
                                self.id,
                                self.count + 1
                            );
                            new_reconnector.run();
                            // return so only on success will the control sockets be notified of changes
                            return;
                        }
                    }
                },
                None => {  },
            }
            println!("Sending reader updates to connected sockets.");
            if let Ok(c_socks) = self.control_sockets.lock() {
                for sock in c_socks.iter() {
                    if let Some(sock) = sock {
                        _ = socket::write_reader_list(&sock, &readers);
                    }
                }
            }
        }
    }
}