pub fn parse_float(input: &str) -> Option<f64> {
    let trimmed = input.trim();

    let mut end = 0;
    for (i, c) in trimmed.char_indices() {
        if c.is_ascii_digit() || c == '.' || (i == 0 && (c == '+' || c == '-')) {
            end = i + 1;
        } else {
            break;
        }
    }

    let valid_part = &trimmed[..end];
    valid_part.parse::<f64>().ok()
}
