fn double(x: u8) -> u8 {
    return x * 2;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_double() {
        assert_eq!(double(2), 4);
    }
}
