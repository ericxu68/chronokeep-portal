use serde::Deserialize;

use crate::objects::setting::Setting;

#[derive(Deserialize, Debug)]
#[serde(tag="command", rename_all="snake_case")]
pub enum Request {
    Unknown,
    // Api related requests
    ApiAdd {
        name: String,
        kind: String,
        uri: String,
        token: String,
    },
    ApiList,
    ApiRemoteAutoUpload {
        name: String,
    },
    ApiRemoteManualUpload {
        name: String,
    },
    ApiRemove {
        name: String,
    },
    ApiResultsEventsGet {
        name: String,
    },
    ApiResultsParticipantsGet {
        api_name: String,
        event_slug: String,
        event_year: String,
    },
    // Connection or program related requests
    Connect,
    Disconnect,
    KeepaliveAck,
    Quit,
    // Participants related requests
    ParticipantsGet,
    ParticipantsRemove,
    // Reader related requests
    ReaderAdd {
        name: String,
        kind: String,
        ip_address: String,
        port: u16,
    },
    ReaderConnect {
        id: i64,
    },
    ReaderDisconnect {
        id: i64,
    },
    ReaderList,
    ReaderRemove {
        id: i64,
    },
    ReaderStart {
        id: i64,
    },
    ReaderStop {
        id: i64,
    },
    // Reads related requests
    ReadsDeleteAll,
    ReadsDelete {
        start_seconds: u64,
        end_seconds: u64,
    },
    ReadsGetAll,
    ReadsGet {
        start_seconds: u64,
        end_seconds: u64,
    },
    // Settings related requests
    SettingSet {
        setting: Setting
    },
    SettingsGet,
    // Subscription request to subscribe to new reads/sightings.
    Subscribe {
        reads: bool,
        sightings: bool,
    },
    // Time related requests
    TimeGet,
    TimeSet {
        time: String,
    },
}