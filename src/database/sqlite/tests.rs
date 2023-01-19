use core::panic;
use std::sync;
use std::fs;
use super::SQLite;
use crate::database::DBError;
use crate::database::Database;
use crate::network::results;
use crate::objects::participant;
use crate::objects::read;
use crate::objects::setting;
use crate::reader::{self, Reader, zebra};

mod test_reader;

fn setup_tests(path: &str) -> SQLite {
    let new_conn = rusqlite::Connection::open(path).unwrap();
    let drop_tables = [
        "DROP TABLE IF EXISTS results_api",
        "DROP TABLE IF EXISTS participants",
        "DROP TABLE IF EXISTS readers",
        "DROP TABLE IF EXISTS chip_reads",
        "DROP TABLE IF EXISTS settings;",
    ];
    for table in drop_tables {
        if let Err(v) = new_conn.execute(table, []) {
            println!("Something went wrong while dropping a table! {table} {v}");
            panic!();
        }
    }
    let output = SQLite {
        mutex: sync::Mutex::new(new_conn)
    };
    match output.setup() {
        Ok(_) => {},
        Err(e) => {
            println!("something went wrong during setup: {e}");
            panic!()
        }
    };
    output
}

fn finalize_tests(path: &str) {
    _ = fs::remove_file(path).is_ok();
}

fn setup_v1(path: &str) -> SQLite {
    let mut new_conn = rusqlite::Connection::open(path).unwrap();
    if let Ok(tx) = new_conn.transaction() {
        let database_tables = [
            "CREATE TABLE IF NOT EXISTS settings (
                setting VARCHAR NOT NULL,
                value VARCHAR NOT NULL,
                UNIQUE (setting) ON CONFLICT REPLACE
            );",
            "CREATE TABLE IF NOT EXISTS results_api (
                id INTEGER PRIMARY KEY,
                nickname VARCHAR(75),
                kind VARCHAR(50),
                token VARCHAR(100),
                uri VARCHAR(150),
                UNIQUE (uri, token) ON CONFLICT REPLACE
            );",
            "CREATE TABLE IF NOT EXISTS participants (
                id INTEGER PRIMARY KEY,
                bib VARCHAR(50) NOT NULL,
                first VARCHAR(50) NOT NULL,
                last VARCHAR(75) NOT NULL,
                age INTEGER NOT NULL DEFAULT 0,
                gender VARCHAR(10) NOT NULL DEFAULT 'u',
                age_group VARCHAR(100) NOT NULL,
                distance VARCHAR(75) NOT NULL,
                part_chip VARCHAR(100) NOT NULL UNIQUE,
                anonymous SMALLINT NOT NULL DEFAULT 0,
                UNIQUE (bib, first, last, distance) ON CONFLICT REPLACE
            );",
            "CREATE TABLE IF NOT EXISTS readers (
                id INTEGER PRIMARY KEY,
                nickname VARCHAR(75) NOT NULL,
                kind VARCHAR(50) NOT NULL,
                ip_address VARCHAR(100) NOT NULL,
                port INTEGER NOT NULL,
                UNIQUE (nickname) ON CONFLICT REPLACE
            );",
            "CREATE TABLE IF NOT EXISTS chip_reads (
                id INTEGER PRIMARY KEY,
                chip VARCHAR(100) NOT NULL,
                seconds BIGINT NOT NULL,
                milliseconds INTEGER NOT NULL,
                antenna INTEGER,
                reader VARCHAR(75),
                rssi VARCHAR(10),
                status INTEGER NOT NULL DEFAULT 0,
                UNIQUE (chip, seconds, milliseconds) ON CONFLICT IGNORE
            );"
        ];
        for table in database_tables {
            if let Err(e) = tx.execute(table, ()) {
                panic!("{}", e)
            }
        }
        if let Err(e) = tx.execute(
            "INSERT INTO settings (setting, value) VALUES (?1, ?2);",
            (super::DATABASE_VERSION_SETTING, super::DATABASE_VERSION.to_string())
        ) {
            panic!("{}", e)
        }
        if let Err(e) = tx.commit() {
            panic!("{}", e)
        }
    }
    SQLite {
        mutex: sync::Mutex::new(new_conn)
    }
}

#[test]
fn test_setup() {
    let unique_path = "./test_setup.sqlite";
    {
        let new_conn = rusqlite::Connection::open(unique_path);
        assert!(new_conn.is_ok());
        let sqlite = SQLite {
            mutex: sync::Mutex::new(new_conn.unwrap())
        };
        let res = sqlite.setup();
        match res {
            Ok(_) => println!("Everything went ok!"),
            Err(e) => {
                println!("Something went wrong! {}", e.to_string());
                panic!();
            }
        }
    }
    finalize_tests(unique_path);
}

#[test]
fn test_update() {
    let unique_path = "./test_update.sqlite";
    {
        let sqlite = setup_v1(unique_path);
        let mut conn = match sqlite.mutex.lock() {
            Ok(c) => c,
            Err(e) => panic!("{}", e)
        };
        match sqlite.update(&mut conn, 1, 1) {
            Ok(_) => println!("Everything went ok!"),
            Err(e) => {
                println!("Something went wrong! {}", e);
                panic!();
            }
        }
    }
    finalize_tests(unique_path);
}

#[test]
fn test_set_setting() {
    let unique_path = "./test_set_setting.sqlite";
    let setting_name = "RANDOM_SETTING";
    let setting_value = "random_value";
    let setting_updated_value = "new_random_value";

    let sqlite = setup_tests(unique_path);

    let num = sqlite.set_setting(&setting::Setting::new(String::from(setting_name), String::from(setting_value)));
    match num {
        Ok(setting) => {
            assert_eq!(setting_name, setting.name());
            assert_eq!(setting_value, setting.value());
        },
        Err(e) => {
            println!("Something went wrong! {}", e);
            panic!()
        }
    }
    // make sure updating doesn't cause an error
    let num = sqlite.set_setting(&setting::Setting::new(String::from(setting_name), String::from(setting_updated_value)));
    match num {
        Ok(setting) => {
            assert_eq!(setting_name, setting.name());
            assert_eq!(setting_updated_value, setting.value());
        },
        Err(e) => {
            println!("Something went wrong! {}", e);
            panic!()
        }
    }
    drop(sqlite);
    finalize_tests(unique_path);
}

#[test]
fn test_get_setting() {
    let unique_path = "./test_get_setting.sqlite";
    let setting_name = "RANDOM_SETTING";
    let setting_value = "random_value";
    let setting_updated_value = "new_random_value";

    let sqlite = setup_tests(unique_path);

    assert!(sqlite.set_setting(&setting::Setting::new(String::from(setting_name), String::from(setting_value))).is_ok());
    let setting = sqlite.get_setting(setting_name);
    match setting {
        Ok(setting) => {
            assert_eq!(setting_name, setting.name());
            assert_eq!(setting_value, setting.value());
        },
        Err(e) => {
            println!("Something went wrong! {}", e);
            panic!()
        }
    }
    // verify that the update function of set_setting works
    assert!(sqlite.set_setting(&setting::Setting::new(String::from(setting_name), String::from(setting_updated_value))).is_ok());
    let setting = sqlite.get_setting(setting_name);
    match setting {
        Ok(setting) => {
            assert_eq!(setting_name, setting.name());
            assert_eq!(setting_updated_value, setting.value());
        },
        Err(e) => {
            println!("Something went wrong! {}", e);
            panic!()
        }
    }
    drop(sqlite);
    finalize_tests(unique_path);
}

#[test]
fn test_save_reader() {
    let unique_path = "./test_save_reader.sqlite";
    let original = zebra::Zebra::new(
        0,
        String::from("zebra-1"),
        String::from("192.168.1.100"),
        zebra::DEFAULT_ZEBRA_PORT);
    let sqlite = setup_tests(unique_path);
    let result = sqlite.save_reader(&original);
    assert!(result.is_ok());
    assert_eq!(1, result.unwrap());
    let readers = sqlite.get_readers().unwrap();
    assert_eq!(1, readers.len());
    let first = readers.first().unwrap();
    assert_eq!(original.nickname(), first.nickname());
    assert_eq!(original.kind(), first.kind());
    assert_eq!(original.ip_address(), first.ip_address());
    assert_eq!(original.port(), first.port());
    // Test auto update feature of the 
    let updated_ip = "random_ip";
    let updated_port = 12345;
    let result = sqlite.save_reader(&zebra::Zebra::new(
            0,
            String::from(original.nickname()),
            String::from(updated_ip),
            updated_port
        ));
    assert!(result.is_ok());
    assert_eq!(1, result.unwrap());
    let readers = sqlite.get_readers().unwrap();
    assert_eq!(1, readers.len());
    let first = readers.first().unwrap();
    assert_eq!(original.nickname(), first.nickname());
    assert_eq!(original.kind(), first.kind());
    assert_eq!(updated_ip, first.ip_address());
    assert_eq!(updated_port, first.port());
    // Test invalid reader kind
    let result = sqlite.save_reader(&test_reader::TestReader::new(
        String::from(original.nickname()),
        String::from("random_type"),
        String::from(updated_ip),
        updated_port
    ));
    assert!(result.is_err());
    match result {
        Ok(_) => panic!(""),
        Err(DBError::DataInsertionError(_)) => println!("Data check verified."),
        Err(e) => {
            println!("Some other error occurred: {e}");
            panic!();
        }
    }
    drop(sqlite);
    finalize_tests(unique_path);
}

#[test]
fn test_get_readers() {
    let unique_path = "./test_get_readers.sqlite";
    let original = zebra::Zebra::new(
        0,
        String::from("zebra-1"),
        String::from("192.168.1.101"),
        zebra::DEFAULT_ZEBRA_PORT + 1);
    let sqlite = setup_tests(unique_path);
    _ = sqlite.save_reader(&original);
    let results = sqlite.get_readers();
    assert!(results.is_ok());
    let readers = results.unwrap();
    assert_eq!(1, readers.len());
    let first = readers.first().unwrap();
    assert!(first.equal(&original));
    // add a bunch of readers to test that we can get them all
    for i in 2..8 {
        _ = sqlite.save_reader(&zebra::Zebra::new(
            0,
            format!("zebra-{i}"),
            format!("192.168.1.10{i}"),
            zebra::DEFAULT_ZEBRA_PORT + i));
    }
    let results = sqlite.get_readers();
    assert!(results.is_ok());
    let readers = results.unwrap();
    assert_eq!(7, readers.len());
    for reader in readers {
        let num = reader.port() - zebra::DEFAULT_ZEBRA_PORT;
        assert_eq!(format!("zebra-{num}"), reader.nickname());
        assert_eq!(reader::READER_KIND_ZEBRA, reader.kind());
        assert_eq!(format!("192.168.1.10{num}"), reader.ip_address());
    }
    drop(sqlite);
    finalize_tests(unique_path);
}

#[test]
fn test_delete_reader() {
    let unique_path = "./test_delete_reader.sqlite";
    let original = zebra::Zebra::new(
        0,
        String::from("zebra-1"),
        String::from("192.168.1.101"),
        zebra::DEFAULT_ZEBRA_PORT + 1);
    let sqlite = setup_tests(unique_path);
    _ = sqlite.save_reader(&original);
    let readers = sqlite.get_readers().unwrap();
    assert_eq!(1, readers.len());
    let first = readers.first().unwrap();
    assert!(first.equal(&original));
    let result = sqlite.delete_reader(original.nickname());
    assert!(result.is_ok());
    assert_eq!(1, result.unwrap());
    let readers = sqlite.get_readers().unwrap();
    assert_eq!(0, readers.len());
    let result = sqlite.delete_reader(original.nickname());
    assert!(result.is_ok());
    assert_eq!(0, result.unwrap());
    // test delete of a single element
    let middle = 4;
    for i in 0..(middle * 2) {
        _ = sqlite.save_reader(&zebra::Zebra::new(
            0,
            format!("zebra-{i}"),
            format!("192.168.1.10{i}"),
            zebra::DEFAULT_ZEBRA_PORT
        ))
    }
    let readers = sqlite.get_readers().unwrap();
    assert_eq!(middle*2, readers.len());
    let result = sqlite.delete_reader(&format!("zebra-{middle}"));
    assert!(result.is_ok());
    assert_eq!(1, result.unwrap());
    let readers = sqlite.get_readers().unwrap();
    assert_eq!((middle*2)-1, readers.len());
    drop(sqlite);
    finalize_tests(unique_path);
}

#[test]
fn test_save_api() {
    let unique_path = "./test_save_api.sqlite";
    let original = results::ResultsApi::new(
        0,
        String::from("results-api"),
        String::from(results::API_TYPE_CHRONOKEEP),
        String::from("random-token-value"),
        String::from("https:://example.com/"));
    let sqlite = setup_tests(unique_path);
    let results = sqlite.save_api(&original);
    assert!(results.is_ok());
    assert_eq!(1, results.unwrap());
    let apis = sqlite.get_apis().unwrap();
    assert_eq!(1, apis.len());
    let first = apis.first().unwrap();
    assert!(first.equal(&original));
    // test update functionality (nickname stays the same)
    let results = sqlite.save_api(
        &results::ResultsApi::new(
            0,
            String::from(original.nickname()),
            String::from(results::API_TYPE_CKEEP_SELF),
            String::from("a-different-random-token"),
            String::from("https:://random.com/")
        ));
    assert!(results.is_ok());
    assert_eq!(1, results.unwrap());
    let apis = sqlite.get_apis().unwrap();
    assert_eq!(1, apis.len());
    let first = apis.first().unwrap();
    assert_eq!(original.nickname(), first.nickname());
    assert_ne!(original.kind(), first.kind());
    assert_ne!(original.token(), first.token());
    assert_ne!(original.uri(), first.uri());
    // test update functionality (token and uri stays the same)
    // save original and verify it updated back
    _ = sqlite.save_api(&original);
    let apis = sqlite.get_apis().unwrap();
    assert_eq!(1, apis.len());
    let first = apis.first().unwrap();
    assert!(first.equal(&original));
    // save new entry
    let results = sqlite.save_api(&results::ResultsApi::new(
        0,
        String::from("new-nickname"),
        String::from(results::API_TYPE_CKEEP_SELF),
        String::from(original.token()),
        String::from(original.uri())
    ));
    assert!(results.is_ok());
    assert_eq!(1, results.unwrap());
    let apis = sqlite.get_apis().unwrap();
    assert_eq!(1, apis.len());
    let first = apis.first().unwrap();
    assert_ne!(original.nickname(), first.nickname());
    assert_ne!(original.kind(), first.kind());
    assert_eq!(original.token(), first.token());
    assert_eq!(original.uri(), first.uri());
    // attempt to save invalid type
    let result = sqlite.save_api(&results::ResultsApi::new(
        0,
        String::from("invalid_type_name"),
        String::from("invalid-type"),
        String::from("random-token"),
        String::from("https:://invalid-type.com/")
    ));
    assert!(result.is_err());
    match result {
        Ok(_) => {
            println!("Expected an error...");
            panic!();
        },
        Err(DBError::DataInsertionError(_)) => println!("Expected error found!"),
        Err(e) => {
            println!("Unexpected error found... {}", e);
            panic!();
        }
    }
    drop(sqlite);
    finalize_tests(unique_path);;
}

#[test]
fn test_get_apis() {
    let unique_path = "./test_get_apis.sqlite";
    let original = results::ResultsApi::new(
        0,
        String::from("results-api"),
        String::from(results::API_TYPE_CHRONOKEEP),
        String::from("random-token-value"),
        String::from("https:://example.com/"));
    let sqlite = setup_tests(unique_path);
    _ = sqlite.save_api(&original);
    let result = sqlite.get_apis();
    assert!(result.is_ok());
    let apis = result.unwrap();
    let first = apis.first().unwrap();
    assert!(first.equal(&original));
    // test that we can add a whole bunch of api entries and retrieve them
    for i in 0..5 {
        _ = sqlite.save_api(&results::ResultsApi::new(
            0,
            format!("api-{i}"),
            String::from(results::API_TYPE_CHRONOKEEP),
            format!("token-number-10302031{i}"),
            String::from("https::api.chronokeep.com/")
        ))
    }
    let apis = sqlite.get_apis().unwrap();
    assert_eq!(6, apis.len());
    drop(sqlite);
    finalize_tests(unique_path);
}

#[test]
fn test_delete_api() {
    let unique_path = "./test_delete_api.sqlite";
    let original = results::ResultsApi::new(
        0,
        String::from("results-api"),
        String::from(results::API_TYPE_CHRONOKEEP),
        String::from("random-token-value"),
        String::from("https:://example.com/"));
    let sqlite = setup_tests(unique_path);
    _ = sqlite.save_api(&original);
    let apis = sqlite.get_apis().unwrap();
    assert_eq!(1, apis.len());
    let result = sqlite.delete_api(original.nickname());
    assert!(result.is_ok());
    assert_eq!(1, result.unwrap());
    let apis = sqlite.get_apis().unwrap();
    assert_eq!(0, apis.len());
    let result = sqlite.delete_api(original.nickname());
    assert!(result.is_ok());
    assert_eq!(0, result.unwrap());
    // verify that we only delete one from a list of apis
    for i in 0..10 {
        _ = sqlite.save_api(&results::ResultsApi::new(
            0,
            format!("results-api-{i}"),
            String::from(results::API_TYPE_CHRONOKEEP),
            format!("random-token-value-{i}"),
            String::from("https:://example.com/")
        ))
    }
    let apis = sqlite.get_apis().unwrap();
    assert_eq!(10, apis.len());
    let result = sqlite.delete_api("results-api-5");
    assert!(result.is_ok());
    assert_eq!(1, result.unwrap());
    let apis = sqlite.get_apis().unwrap();
    assert_eq!(9, apis.len());
    drop(sqlite);
    finalize_tests(unique_path);;
}

fn make_reads() -> Vec<read::Read> {
    let mut output: Vec<read::Read> = Vec::new();
    output.push(read::Read::new(
            0,
            String::from("1005"),
            1005,
            100,
            2,
            String::from("reader-1"),
            String::from("-25dba"),
            read::READ_STATUS_USED
    ));
    output.push(read::Read::new(
            0,
            String::from("1005"),
            11005,
            90,
            4,
            String::from("reader-1"),
            String::from("-20dba"),
            read::READ_STATUS_TOO_SOON
    ));
    // this entry should be ignored on save
    output.push(read::Read::new(
            0,
            String::from("1005"),
            1005,
            100,
            3,
            String::from("reader-1"),
            String::from("-5dba"),
            read::READ_STATUS_UNUSED
    ));
    for i in 1006..1100 {
        output.push(read::Read::new(
            0,
            format!("{i}"),
            i,
            100,
            1,
            String::from("reader-1"),
            String::from("-25dba"),
            read::READ_STATUS_UNUSED
        ));
    }
    output
}

#[test]
fn test_save_reads() {
    let unique_path = "./test_save_reads.sqlite";
    let new_reads = make_reads();
    let sqlite = setup_tests(unique_path);
    let result = sqlite.save_reads(&new_reads);
    assert!(result.is_ok());
    assert_eq!(new_reads.len() - 1, result.unwrap());
    // test if we can add a read we already know about, this should return 0
    let temp_read = new_reads.first().unwrap();
    let updated_read = read::Read::new(
        0,
        String::from(temp_read.chip()),
        temp_read.seconds(),
        temp_read.milliseconds(),
        500,
        String::from("new-reader-name"),
        String::from("15dba"),
        read::READ_STATUS_USED
    );
    let result = sqlite.save_reads(&vec![updated_read]);
    assert!(result.is_ok());
    assert_eq!(0, result.unwrap());
    // test if we can add a status that we don't know about
    let updated_read = read::Read::new(
        0,
        String::from(temp_read.chip()),
        temp_read.seconds(),
        temp_read.milliseconds(),
        500,
        String::from("new-reader-name"),
        String::from("15dba"),
        255
    );
    let result = sqlite.save_reads(&vec![updated_read]);
    assert!(result.is_err());
    drop(sqlite);
    finalize_tests(unique_path);;
}

#[test]
fn test_get_reads() {
    let unique_path = "./test_get_reads.sqlite";
    let new_reads = make_reads();
    let sqlite = setup_tests(unique_path);
    _ = sqlite.save_reads(&new_reads);
    let result = sqlite.get_reads(0, 2000);
    assert!(result.is_ok());
    let reads = result.unwrap();
    assert_eq!(new_reads.len()-2, reads.len());
    for outer in reads.iter() {
        let mut found = false;
        for inner in new_reads.iter() {
            if outer.equal(&inner) {
                found = true
            }
        }
        assert!(found)
    }
    drop(sqlite);
    finalize_tests(unique_path);;
}

#[test]
fn test_delete_reads() {
    let unique_path = "./test_delete_reads.sqlite";
    let new_reads = make_reads();
    let sqlite = setup_tests(unique_path);
    let count = sqlite.save_reads(&new_reads).unwrap();
    let result = sqlite.delete_reads(2000, 90000);
    assert!(result.is_ok());
    assert_eq!(1, result.unwrap());
    let reads = sqlite.get_reads(0, 90000).unwrap();
    assert_eq!(count-1, reads.len());
    let result = sqlite.delete_reads(0, 2000);
    assert!(result.is_ok());
    assert_eq!(count-1, result.unwrap());
    let reads = sqlite.get_reads(0, 90000).unwrap();
    assert_eq!(0, reads.len());
    let result = sqlite.delete_reads(0, 90000);
    assert!(result.is_ok());
    assert_eq!(0, result.unwrap());
    drop(sqlite);
    finalize_tests(unique_path);
}

fn make_participants() -> Vec<participant::Participant> {
    let mut output: Vec<participant::Participant> = Vec::new();
    output.push(participant::Participant::new(
        0,
        String::from("1005"),
        String::from(""),
        String::from(""),
        50,
        String::from("F"),
        String::from("0-110"),
        String::from("50k"),
        String::from("1005"),
        true)
    );
    output.push(participant::Participant::new(
        0,
        String::from("1006"),
        String::from("John"),
        String::from("Smith"),
        22,
        String::from("M"),
        String::from("0-110"),
        String::from("50k"),
        String::from("1006"),
        false)
    );
    output.push(participant::Participant::new(
        0,
        String::from("1007"),
        String::from("Jenny"),
        String::from("Appfelsauce"),
        34,
        String::from("F"),
        String::from("0-110"),
        String::from("50k"),
        String::from("1007"),
        false)
    );
    output.push(participant::Participant::new(
        0,
        String::from("1008"),
        String::from("Jon"),
        String::from("Johnson"),
        20,
        String::from("NB"),
        String::from("0-110"),
        String::from("50k"),
        String::from("1008"),
        false)
    );
    output.push(participant::Participant::new(
        0,
        String::from("1009"),
        String::from("George"),
        String::from("Analabousch"),
        65,
        String::from("U"),
        String::from("0-110"),
        String::from("50k"),
        String::from("1009"),
        false)
    );
    output
}

#[test]
fn test_add_participants() {
    let unique_path = "./test_add_participants.sqlite";
    let participants = make_participants();
    let sqlite = setup_tests(unique_path);
    let result = sqlite.add_participants(&participants);
    assert!(result.is_ok());
    assert_eq!(participants.len(), result.unwrap());
    let parts = sqlite.get_participants().unwrap();
    for outer in participants.iter() {
        let mut found = false;
        for inner in parts.iter() {
            if outer.equals(&inner) {
                found = true;
            }
        }
        assert!(found)
    }
    // test update / bib & chip collisions
    let new_part = vec!(participant::Participant::new(
        0,
        String::from("1009"),
        String::from("Updated First"),
        String::from("Updated Last"),
        3,
        String::from("M"),
        String::from("0-110"),
        String::from("50k"),
        String::from("1006"),
        false
    ));
    let result = sqlite.add_participants(&new_part);
    assert!(result.is_ok());
    assert_eq!(1, result.unwrap());
    let parts = sqlite.get_participants().unwrap();
    // this should have replaced two entries, bib 1009 and chip 1006
    assert_eq!(participants.len()-1, parts.len());
    let mut found = false;
    let np = new_part.first().unwrap();
    for p in parts {
        if np.equals(&p) {
            found = true;
            break;
        }
    }
    assert!(found);
    drop(sqlite);
    finalize_tests(unique_path);
}

#[test]
fn test_delete_participants() {
    let unique_path = "./test_delete_participants.sqlite";
    let participants = make_participants();
    let sqlite = setup_tests(unique_path);
    _ = sqlite.add_participants(&participants);
    let parts = sqlite.get_participants().unwrap();
    assert_eq!(participants.len(), parts.len());
    let result = sqlite.delete_participants();
    assert!(result.is_ok());
    assert_eq!(participants.len(), result.unwrap());
    let parts = sqlite.get_participants().unwrap();
    assert_eq!(0, parts.len());
    drop(sqlite);
    finalize_tests(unique_path);
}

#[test]
fn test_delete_participant() {
    let unique_path = "./test_delete_participant.sqlite";
    let participants = make_participants();
    let sqlite = setup_tests(unique_path);
    _ = sqlite.add_participants(&participants);
    let parts = sqlite.get_participants().unwrap();
    assert_eq!(participants.len(), parts.len());
    // delete known bib
    let result = sqlite.delete_participant("1009");
    assert!(result.is_ok());
    assert_eq!(1, result.unwrap());
    let parts = sqlite.get_participants().unwrap();
    assert_eq!(participants.len()-1, parts.len());
    // try to delete again
    let result = sqlite.delete_participant("1009");
    assert!(result.is_ok());
    assert_eq!(0, result.unwrap());
    let parts = sqlite.get_participants().unwrap();
    assert_eq!(participants.len()-1, parts.len());
    // test unknown bib
    let result = sqlite.delete_participant("invalid");
    assert!(result.is_ok());
    assert_eq!(0, result.unwrap());
    let parts = sqlite.get_participants().unwrap();
    assert_eq!(participants.len()-1, parts.len());
    drop(sqlite);
    finalize_tests(unique_path);
}