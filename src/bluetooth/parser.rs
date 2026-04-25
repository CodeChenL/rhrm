pub fn parse_heart_rate(data: &[u8]) -> Option<u16> {
    if data.is_empty() {
        return None;
    }

    let flag = data[0];
    if data.len() < 2 {
        return None;
    }

    let mut heart_rate = data[1] as u16;
    if flag & 0b00001 != 0 && data.len() >= 3 {
        heart_rate |= (data[2] as u16) << 8;
    }
    Some(heart_rate)
}

#[cfg(test)]
mod tests {
    use super::parse_heart_rate;

    #[test]
    fn parse_empty_packet_returns_none() {
        assert_eq!(parse_heart_rate(&[]), None);
    }

    #[test]
    fn parse_eight_bit_packet() {
        assert_eq!(parse_heart_rate(&[0x00, 72]), Some(72));
    }

    #[test]
    fn parse_sixteen_bit_packet() {
        assert_eq!(parse_heart_rate(&[0x01, 0x34, 0x12]), Some(0x1234));
    }
}
