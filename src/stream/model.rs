use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct StreamMessage {
    pub part: String,
    pub proto: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dev: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "stream")]
    pub record: Option<Vec<u8>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SequencedRecord {
    pub seq: u32,
    #[serde(rename = "data")]
    pub raw_data: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Record {
    #[serde(rename = "capSense")]
    CapSense(CapSense),
    /// this is never sent on my pod
    #[serde(rename = "piezo-dual")]
    PiezoDual(PiezoDual),
    #[serde(rename = "piezo-sub")]
    PiezoSub(PiezoSub),
    #[serde(rename = "bedTemp")]
    BedTemp(BedTemp),
    #[serde(rename = "log")]
    Log(Log),
    #[serde(rename = "frzTemp")]
    FrzTemp(FrzTemp),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CapSense {
    pub ts: i64,
    pub left: CapSenseSide,
    pub right: CapSenseSide,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CapSenseSide {
    pub status: String,
    pub cen: u16,
    #[serde(rename = "in")]
    pub in_: u16,
    pub out: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PiezoDual {
    pub ts: i64,
    pub adc: u8,
    pub freq: u16,
    pub gain: u16,
    #[serde(with = "serde_bytes")]
    pub left1: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub left2: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub right1: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub right2: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PiezoSub {
    pub ts: i64,
    pub freq: u16,
    pub part: i16,
    pub left: PiezoSubSide,
    pub right: PiezoSubSide,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PiezoSubSide {
    pub gain: u16,
    #[serde(with = "serde_bytes")]
    pub samples: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BedTempSide {
    pub cen: u16,
    #[serde(rename = "in")]
    pub in_: u16,
    pub out: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BedTemp {
    pub ts: i64,
    pub mcu: u16,
    pub amb: u16,
    pub hu: u16,
    pub left: BedTempSide,
    pub right: BedTempSide,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FrzTemp {
    pub ts: i64,
    pub amb: u16,
    pub hs: u16,
    pub left: u16,
    pub right: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Log {
    pub ts: i64,
    pub msg: String,
    pub level: String,
}
