use std::{sync::{Arc, Mutex, Condvar}, net::TcpStream, collections::HashMap, str::FromStr};

use crate::{control::{socket::{MAX_CONNECTED, self}, SETTING_SIGHTING_PERIOD}, database::{sqlite, Database}, objects::{read::{self}, participant, sighting}, defaults::DEFAULT_SIGHTING_PERIOD};

pub struct SightingsProcessor {
    control_sockets: Arc<Mutex<[Option<TcpStream>;MAX_CONNECTED + 1]>>,
    sighting_repeaters: Arc<Mutex<[bool;MAX_CONNECTED]>>,

    sqlite: Arc<Mutex<sqlite::SQLite>>,

    keepalive: Arc<Mutex<bool>>,
    running: Arc<Mutex<bool>>,

    semaphore: Arc<(Mutex<bool>, Condvar)>
}

impl SightingsProcessor {
    pub fn new(
        control_sockets: Arc<Mutex<[Option<TcpStream>;MAX_CONNECTED+1]>>,
        sighting_repeaters: Arc<Mutex<[bool;MAX_CONNECTED]>>,
        sqlite: Arc<Mutex<sqlite::SQLite>>,
        keepalive: Arc<Mutex<bool>>
    ) -> SightingsProcessor {
        return SightingsProcessor {
            control_sockets,
            sighting_repeaters,
            sqlite,
            keepalive,
            running: Arc::new(Mutex::new(false)),
            semaphore: Arc::new((Mutex::new(false), Condvar::new()))
        }
    }

    pub fn notify(&self) {
        let (lock, cvar) = &*self.semaphore;
        let mut notify = lock.lock().unwrap();
        *notify = true;
        cvar.notify_all()
    }

    pub fn stop(&self) {
        println!("Sending shutdown command to sightings processor.");
        if let Ok(mut run) = self.running.lock() {
            *run = false;
        }
    }

    pub fn running(&self) -> bool {
        if let Ok(run) = self.running.lock() {
            return *run
        }
        false
    }

    pub fn start(&self) {
        if let Ok(mut run) = self.running.lock() {
            *run = true
        } else {
            return
        }
        println!("Starting sightings processor.");
        'main: loop {
            if let Ok(ka) = self.keepalive.lock() {
                if *ka == false {
                    println!("Sightings processor told to quit. /1/");
                    break;
                }
            } else {
                println!("Error getting keep alive mutex. Exiting.");
                break;
            }
            if let Ok(run) = self.running.lock() {
                if *run == false {
                    println!("Sightings processor told to quit. /2/");
                    break;
                }
            }
            let (lock, cvar) = &*self.semaphore;
            match cvar.wait_while(
                lock.lock().unwrap(),
                |notify| *notify == false
            ) {
                Ok(_) => {
                    // once we've been notified, keep processing reads until there's nothing left to do
                    loop {
                        let reads: Vec<read::Read>;
                        let parts: Vec<participant::Participant>;
                        if let Ok(sq) = self.sqlite.lock() {
                            reads = match sq.get_useful_reads() {
                                Ok(r) => r,
                                Err(e) => {
                                    println!("error getting useful reads: {e}");
                                    break 'main;
                                }
                            };
                            parts = match sq.get_participants() {
                                Ok(p) => p,
                                Err(e) => {
                                    println!("error getting participants: {e}");
                                    break 'main;
                                }
                            };
                        } else {
                            println!("error getting sqlite database lock");
                            break 'main;
                        }
                        // sort values into unused reads and the last read we've seen from a person
                        let mut unused: Vec<read::Read> = Vec::new();
                        let mut used: HashMap<String, read::Read> = HashMap::new();
                        // create a hashmap for changing bibs to chips
                        let mut bibChipMap: HashMap<String, String> = HashMap::new();
                        // make a map of participants based upon their chip
                        let mut part_map: HashMap<String, participant::Participant> = HashMap::new();
                        for part in parts {
                            bibChipMap.insert(String::from(part.bib()), String::from(part.chip()));
                            let chip = String::from(part.chip());
                            part_map.insert(chip, part);
                        }
                        for read in reads {
                            if read.status() == read::READ_STATUS_UNUSED {
                                unused.push(read);
                            } else if read.status() == read::READ_STATUS_USED {
                                // if the identifier type is a bib we need to get the chip from our bibChipMap by
                                // the bib stored in the chip() value of read
                                let mut chip = String::from(read.chip());
                                match read.ident_type() {
                                    read::READ_IDENT_TYPE_BIB => {
                                        let bib = String::from(read.chip());
                                        if bibChipMap.contains_key(&bib) {
                                            chip = bibChipMap[&bib].clone();
                                        }
                                    }
                                    read::READ_IDENT_TYPE_CHIP => {}
                                    e => {
                                        println!("Error occurred during sightings processing. Unknown read identifier type. {e}");
                                    }
                                }
                                if used.contains_key(&chip) {
                                    let last = &used[&chip];
                                    if last.seconds() < read.seconds() ||
                                        (last.seconds() == read.seconds() && last.milliseconds() < read.milliseconds())
                                    {
                                        used.insert(chip, read);
                                    }
                                } else {
                                    used.insert(chip, read);
                                }
                            }
                        }
                        // if nothing left to process, we can exit
                        if unused.len() == 0 {
                            break;
                        }
                        // sort all the unused reads by second
                        unused.sort_by(|a, b|
                            if a.seconds() == b.seconds() {
                                a.milliseconds().cmp(&b.milliseconds())
                            } else {
                                a.seconds().cmp(&b.seconds())
                            }
                        );
                        let mut period = DEFAULT_SIGHTING_PERIOD as u64;
                        if let Ok(sq) = self.sqlite.lock() {
                            match sq.get_setting(SETTING_SIGHTING_PERIOD) {
                                Ok(setting) => {
                                    period = u64::from_str(setting.value()).unwrap();
                                }
                                Err(e) => {
                                    println!("error getting sighting period: {e}");
                                }
                            }
                        }
                        // these vecs need to be added to the database
                        let mut upd_reads: Vec<read::Read> = Vec::new();
                        let mut upd_parts: Vec<participant::Participant> = Vec::new();
                        let mut sightings: Vec<sighting::Sighting> = Vec::new();
                        for mut read in unused {
                            // if the identifier type is a bib we need to get the chip from our bibChipMap by
                            // the bib stored in the chip() value of read
                            let mut chip = String::from(read.chip());
                            match read.ident_type() {
                                read::READ_IDENT_TYPE_BIB => {
                                    let bib = String::from(read.chip());
                                    if bibChipMap.contains_key(&bib) {
                                        chip = bibChipMap[&bib].clone();
                                    }
                                }
                                read::READ_IDENT_TYPE_CHIP => {}
                                e => {
                                    println!("Error occurred during sightings processing. Unknown read identifier type. {e}");
                                }
                            }
                            if part_map.contains_key(&chip) == false {
                                let new_part = participant::Participant::new(
                                    0,
                                    chip.clone(),
                                    String::from("J"),
                                    String::from("Doe"),
                                    0,
                                    String::from("U"),
                                    String::from("0-110"),
                                    String::from("Unknown"),
                                    chip.clone(),
                                    false
                                );
                                upd_parts.push(new_part.clone());
                                part_map.insert(chip.clone(), new_part);
                            }
                            // check if we're within the period where we should ignore the read
                            if used.contains_key(&chip) {
                                let tmp = &used[&chip];
                                // not out of ignore period
                                if tmp.seconds() + period > read.seconds() {
                                    read.set_status(read::READ_STATUS_TOO_SOON);
                                    upd_reads.push(read);
                                // barely in the ignore period
                                } else if tmp.seconds() + period == read.seconds() && tmp.milliseconds() > read.milliseconds() {
                                    read.set_status(read::READ_STATUS_TOO_SOON);
                                    upd_reads.push(read);
                                // not in the ignore period
                                } else {
                                    // update the read
                                    read.set_status(read::READ_STATUS_USED);
                                    upd_reads.push(read.clone());
                                    // update the map
                                    used.insert(chip.clone(), read.clone());
                                    // get the participant
                                    let part = &part_map[&chip];
                                    sightings.push(sighting::Sighting {
                                        participant: part.clone(),
                                        read
                                    });
                                }
                            // nothing in the used map
                            } else {
                                // update the read
                                read.set_status(read::READ_STATUS_USED);
                                upd_reads.push(read.clone());
                                // update the map
                                used.insert(chip.clone(), read.clone());
                                // get the participant
                                let part = &part_map[&chip];
                                sightings.push(sighting::Sighting {
                                    participant: part.clone(),
                                    read
                                });
                            }
                        }
                        if let Ok(mut sq) = self.sqlite.lock() {
                            if upd_parts.len() > 0 {
                                match sq.add_participants(&upd_parts) {
                                    Ok(_) => (),
                                    Err(e) => {
                                        println!("error adding participants: {e}");
                                        break 'main;
                                    }
                                }
                                let participants = match sq.get_participants() {
                                    Ok(p) => p,
                                    Err(e) => {
                                        println!("error getting participants: {e}");
                                        break 'main;
                                    }
                                };
                                // update part map so we have id's for any participants we added
                                for part in participants {
                                    let chip = String::from(part.chip());
                                    part_map.insert(chip, part);
                                }
                                // update all the sightings
                                let tmp_sightings = Vec::from(sightings);
                                sightings = Vec::new();
                                for mut sight in tmp_sightings {
                                    let chip = String::from(sight.read.chip());
                                    if part_map.contains_key(&chip) {
                                        sight.participant = part_map[&chip].clone();
                                        sightings.push(sight);
                                    } else {
                                        println!("participant not found somehow...");
                                        break 'main;
                                    }
                                }
                            }
                            match sq.update_reads_status(&upd_reads) {
                                Ok(_) => (),
                                Err(e) => {
                                    println!("error updating read statuses: {e}");
                                    break 'main;
                                }
                            }
                            match sq.save_sightings(&sightings) {
                                Ok(_) => (),
                                Err(e) => {
                                    println!("error saving sightings: {e}");
                                    break 'main;
                                }
                            }
                            // send sightings
                            if let Ok(sockets) = self.control_sockets.lock() {
                                if let Ok(repeaters) = self.sighting_repeaters.lock() {
                                    for ix in 0..MAX_CONNECTED {
                                        match &sockets[ix] {
                                            Some(sock) => {
                                                if repeaters[ix] == true {
                                                    socket::write_sightings(&sock, &sightings);
                                                }
                                            },
                                            None => ()
                                        }
                                    }
                                } else {
                                    println!("error getting repeaters mutex");
                                }
                            } else {
                                println!("error getting control sockets mutex");
                            }
                        } else {
                            println!("error getting database to update sightings");
                            break 'main;
                        }
                    }
                },
                Err(e) => {
                    println!("unable to aquire semaphore: {e}");
                    break;
                }
            }
            // set notify mutex to false since we've finished
            if let Ok(mut notify) = lock.lock() {
                *notify = false
            }
        }
        if let Ok(mut run) = self.running.lock() {
            *run = false
        }
    }
}