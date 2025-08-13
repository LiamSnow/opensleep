// Implementation of CRC-CCITT (0x1D0F) referenced from `libstd`

const CRC_START: u16 = 0x1D0F;
const CRC_POLY_CCITT: u16 = 0x1021;
const CRC_TABLE: [u16; 256] = make_crc_table();

/// precompute CRC table
const fn make_crc_table() -> [u16; 256] {
    let mut table = [0u16; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = 0u16;
        let mut c = (i as u16) << 8;
        let mut j = 0;
        while j < 8 {
            if (crc ^ c) & 0x8000 != 0 {
                crc = (crc << 1) ^ CRC_POLY_CCITT;
            } else {
                crc <<= 1;
            }
            c <<= 1;
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

pub const fn compute(input: &[u8]) -> u16 {
    let mut crc = CRC_START;
    let mut i = 0;
    while i < input.len() {
        let byte = input[i];
        let index = ((crc >> 8) ^ (byte as u16)) & 0x00FF;
        crc = (crc << 8) ^ CRC_TABLE[index as usize];
        i += 1;
    }
    crc
}

#[cfg(test)]
mod tests {
    use hex_literal::hex;

    #[test]
    fn test_checksum() {
        assert_eq!(super::compute(&hex!("40 0001 0E10")), 0xE6A8);
        assert_eq!(super::compute(&hex!("40 0101 0A14")), 0x1C5C);
        assert_eq!(super::compute(&hex!("40 0000 1194")), 0x13d9);
        assert_eq!(super::compute(&hex!("40 0000 10e0")), 0x1efb);
        assert_eq!(super::compute(&hex!("40 0000 0d5c")), 0x0d83);
        assert_eq!(super::compute(&hex!("40 0000 0a8c")), 0x5f69);
        assert_eq!(super::compute(&hex!("40 0000 03e8")), 0xc9d3);
        assert_eq!(super::compute(&hex!("40 0000 0834")), 0x1fd8);
    }
}
