use std::{
    io::{self, BufReader, BufWriter, ErrorKind, Write},
    net::{TcpListener, TcpStream},
    thread,
};

use ciborium::de;
use log::{error, info};
use model::{Record, SequencedRecord, StreamMessage};

mod model;

/// ciborium does not allow async, so this runs in its own thread
pub fn run_blocking() {
    let lis = TcpListener::bind("127.0.0.1:1337").unwrap();

    loop {
        match lis.accept() {
            Ok((stream, _)) => {
                thread::spawn(|| stream_task(stream));
            }
            Err(e) => {
                error!("[Stream] Couldn't accept TCP stream: {e}")
            }
        }
    }
}

pub fn stream_task(stream: TcpStream) {
    info!("[Stream] Accepted new TCP stream");

    let mut writer = BufWriter::new(&stream);
    let mut reader = BufReader::new(&stream);

    loop {
        StreamMessage::read(&mut reader).map(|msg| msg.dispatch(&mut writer));
    }
}

impl StreamMessage {
    fn read(reader: &mut BufReader<&TcpStream>) -> Option<Self> {
        match ciborium::from_reader(reader) {
            Ok(m) => Some(m),
            Err(e) => {
                error!("[Stream] Failed to read StreamMessage: {e}");
                None
            }
        }
    }

    fn new_handshake() -> Self {
        Self {
            part: "session".into(),
            proto: "raw".into(),
            ..Default::default()
        }
    }

    fn new_batch_accept(id: u32) -> Self {
        Self {
            part: "batch".into(),
            proto: "raw".into(),
            id: Some(id),
            ..Default::default()
        }
    }

    fn write(&self, writer: &mut BufWriter<&TcpStream>) -> io::Result<()> {
        if let Err(e) =
            ciborium::into_writer::<StreamMessage, &mut BufWriter<&TcpStream>>(self, writer)
        {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("[Stream] Failed making ciborium writer {e}"),
            ));
        }

        writer.flush()
    }

    fn dispatch(self, writer: &mut BufWriter<&TcpStream>) {
        match self.part.as_str() {
            "session" => self.handshake(writer),
            "batch" => self.parse_batch(writer),
            _ => {
                error!("[Stream] Got unexepected stream part {:?}", self.part);
            }
        }
    }

    fn handshake(self, writer: &mut BufWriter<&TcpStream>) {
        info!("[Stream] Frank requested a session: {:#?}", self);
        match Self::new_handshake().write(writer) {
            Ok(_) => {
                info!("[Stream] Session started for {}", self.dev.expect("ERR"));
            }
            Err(e) => {
                error!(
                    "[Stream] Session handshake failed for {}, {e}",
                    self.dev.expect("ERR")
                );
            }
        }
    }

    fn parse_batch(self, writer: &mut BufWriter<&TcpStream>) {
        info!("[Stream] Frank send Batch: proto={},id={:?},version={:?},dev={:?}", self.proto, self.id, self.version, self.dev);

        let (id, record) = match (self.id, self.record) {
            (Some(i), Some(r)) => (i, r),
            _ => {
                error!("[Stream] Ignoring bad Batch (missing ID or record)");
                return;
            }
        };

        if let Err(e) = Self::new_batch_accept(id).write(writer) {
            error!("[Stream] Batch response error: {e}");
            return;
        }

        let mut reader = BufReader::new(record.as_slice());
        while let Some(srec) = SequencedRecord::read(&mut reader) {
            let seq = srec.seq;
            let inp = hex::encode(srec.raw_data.clone());
            // info!("got raw data for seq rec: {inp}");

            let rec = Record::read(&mut srec.raw_data.as_slice());

            if let Some(Record::CapSense(c)) = rec {
                info!("CAP {c:#?}");
            }
        }
    }
}

// TODO skip seq rec step??
impl SequencedRecord {
    fn read(reader: &mut BufReader<&[u8]>) -> Option<Self> {
        match ciborium::from_reader(reader) {
            Ok(r) => Some(r),
            Err(de::Error::Io(error)) if error.kind() == ErrorKind::UnexpectedEof => None,
            Err(e) => {
                error!("[Stream] Failed to read SequencedRecord: {:?}", e);
                None
            }
        }
    }
}

impl Record {
    fn read(reader: &mut &[u8]) -> Option<Self> {
        let inp = hex::encode(&mut *reader);
        match ciborium::from_reader(reader) {
            Ok(r) => Some(r),
            Err(e) => {
                error!("[Stream] Failed to deserialize Record: {e}. Input: {inp}",);
                None
            }
        }
    }
}
