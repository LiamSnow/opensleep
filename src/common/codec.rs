use super::{checksum, packet::Packet};
use bytes::{Buf, BufMut, BytesMut};
use std::marker::PhantomData;
use tokio_util::codec::{Decoder, Encoder};

pub const START: u8 = 0x7E;

pub struct PacketCodec<P: Packet> {
    _phantom: PhantomData<P>,
}

impl<P: Packet> PacketCodec<P> {
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<P: Packet> Default for PacketCodec<P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P: Packet> Decoder for PacketCodec<P> {
    type Item = P;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        loop {
            let start_pos = memchr::memchr(START, src);

            match start_pos {
                Some(pos) => {
                    // skip bytes until pos
                    if pos > 0 {
                        src.advance(pos);
                    }

                    if src.len() < 2 {
                        return Ok(None); // need more data
                    }

                    let len = src[1] as usize;
                    let total_packet_size = 1 + 1 + len + 2; // start + len + payload + checksum

                    if src.len() < total_packet_size {
                        return Ok(None); // need more data
                    }

                    // get payload
                    let payload_start = 2;
                    let payload_end = 2 + len;
                    let payload = &src[payload_start..payload_end];
                    if payload.is_empty() {
                        log::error!("Empty packet");
                        src.advance(1);
                        continue;
                    }

                    // validate checksum wo/ consuming bytes
                    let checksum_bytes = &src[payload_end..payload_end + 2];
                    let actual_checksum =
                        u16::from_be_bytes([checksum_bytes[0], checksum_bytes[1]]);
                    let expected_checksum = checksum::compute(payload);

                    if actual_checksum != expected_checksum {
                        // bad checksum -> skip only start byte and try again
                        src.advance(1);
                        continue;
                    }

                    // checksum is valid -> try to parse packet
                    src.advance(2); // skip start & len
                    let payload = src.split_to(len); // take payload out
                    src.advance(2); // skip checksum

                    match P::parse(payload) {
                        Ok(packet) => {
                            // consume valid packets
                            return Ok(Some(packet));
                        }
                        Err(e) => {
                            log::error!("{e}");
                            continue;
                        }
                    }
                }
                None => {
                    // no start byte found -> clear buffer
                    src.clear();
                    return Ok(None);
                }
            }
        }
    }
}

pub fn command(mut payload: Vec<u8>) -> Vec<u8> {
    let mut res = Vec::with_capacity(payload.len() + 4);
    let checksum = checksum::compute(&payload);
    res.push(START);
    res.push(payload.len() as u8);
    res.append(&mut payload);
    res.push((checksum >> 8) as u8);
    res.push(checksum as u8);
    res
}

pub trait CommandTrait {
    fn to_bytes(&self) -> Vec<u8>;
}

impl<P: Packet, C: CommandTrait> Encoder<C> for PacketCodec<P> {
    type Error = std::io::Error;

    fn encode(&mut self, item: C, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.put_slice(&item.to_bytes());
        Ok(())
    }
}
