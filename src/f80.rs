pub fn f80_to_f64(mut input: u128) -> f64 {
    // Extract parts
    let mut mantissa = input & (1 << 63) - 1;
    input >>= 63;

    // let integer = input & 1;
    input >>= 1;

    let mut exp = input & (1 << 15) - 1;
    input >>= 15;

    let sign = input & 1;
    assert_eq!(input >> 1, 0);

    // Convert base offset
    if exp >= 16383 {
        exp = exp - 16383 + 1023;
    }
    // Truncate end of mantissa
    mantissa >>= 63 - 52;

    // Reassemble parts
    let mut output = 0;

    output |= sign as u64;

    output <<= 11;
    output |= exp as u64;

    output <<= 52;
    output |= mantissa as u64;

    f64::from_bits(output)
}
