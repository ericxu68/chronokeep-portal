use crate::{database::{sqlite, self, Database, DBError}, defaults, objects::setting};

pub const SETTING_SIGHTING_PERIOD: &str = "SETTING_SIGHTING_PERIOD";
pub const SETTING_ZERO_CONF_PORT: &str = "SETTING_ZERO_CONF_PORT";
pub const SETTING_CONTROL_PORT: &str = "SETTING_CONTROL_PORT";

pub struct Control {
    pub sighting_period: u32,
    pub zero_conf_port: u16,
    pub control_port: u16,
}

impl Control {
    pub fn new(sqlite: &sqlite::SQLite) -> Result<Control, database::DBError> {
        let mut output = Control {
            sighting_period: defaults::DEFAULT_SIGHTING_PERIOD,
            zero_conf_port: defaults::DEFAULT_ZERO_CONF_PORT,
            control_port: defaults::DEFAULT_CONTROL_PORT,
        };
        match sqlite.get_setting(SETTING_SIGHTING_PERIOD) {
            Ok(s) => {
                let port: u32 = s.value().parse().unwrap();
                output.sighting_period = port;
            },
            // not found means we use the default
            Err(DBError::NotFound) => {
                match sqlite.set_setting(&setting::Setting::new(
                    String::from(SETTING_SIGHTING_PERIOD),
                    format!("{}", defaults::DEFAULT_SIGHTING_PERIOD)
                )) {
                    Ok(_) => {},
                    Err(e) => return Err(e)
                }
            },
            Err(e) => return Err(e)
        }
        match sqlite.get_setting(SETTING_ZERO_CONF_PORT) {
            Ok(s) => {
                let port: u16 = s.value().parse().unwrap();
                output.zero_conf_port = port;
            },
            // not found means we use the default
            Err(DBError::NotFound) => {
                match sqlite.set_setting(&setting::Setting::new(
                    String::from(SETTING_ZERO_CONF_PORT),
                    format!("{}", defaults::DEFAULT_ZERO_CONF_PORT)
                )) {
                    Ok(_) => {},
                    Err(e) => return Err(e)
                }
            },
            Err(e) => return Err(e)
        }
        match sqlite.get_setting(SETTING_CONTROL_PORT) {
            Ok(s) => {
                let port: u16 = s.value().parse().unwrap();
                output.control_port = port;
            },
            // not found means we use the default
            Err(DBError::NotFound) => {
                match sqlite.set_setting(&setting::Setting::new(
                    String::from(SETTING_CONTROL_PORT),
                    format!("{}", defaults::DEFAULT_CONTROL_PORT)
                )) {
                    Ok(_) => {},
                    Err(e) => return Err(e)
                }
            },
            Err(e) => return Err(e)
        }
        Ok(output)
    }
}