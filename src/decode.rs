fn hex_digit_to_u8(hex_digit: u8) -> u8 {
    match hex_digit {
        b'0'..=b'9' => hex_digit - b'0',
        b'a'..=b'f' => hex_digit - b'a' + 10,
        b'A'..=b'F' => hex_digit - b'A' + 10,
        _ => panic!("Invalid hex character"),
    }
}

pub fn hex_decode(hex: Vec<u8>) -> Vec<u8> {
    hex.chunks_exact(2).map(|chunk| match chunk {
        [high, low] => hex_digit_to_u8(*high) * 16 + hex_digit_to_u8(*low),
        _ => unreachable!("Chucks contain exactly 2 elements"),
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

   #[test]
    fn test_decode() {
       assert_eq!(hex_decode(b"00a742#".to_vec()), b"\x00\xa7\x42");
       assert_eq!(hex_decode(b"00a742".to_vec()), b"\x00\xa7\x42");
       assert_eq!(hex_decode(b"".to_vec()), b"");
   }
}
