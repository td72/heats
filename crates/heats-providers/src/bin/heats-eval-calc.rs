use std::io::BufRead;

/// Convert bare integer literals to float literals (e.g. "1/3" → "1.0/3.0")
/// so evalexpr performs float division instead of integer division.
/// Already-float literals like "3.14" are left unchanged.
fn intlit_to_float(expr: &str) -> String {
    let mut result = String::with_capacity(expr.len() + 8);
    let bytes = expr.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            // If followed by '.', it's already a float — consume the fractional part too
            if i < bytes.len() && bytes[i] == b'.' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
                i += 1; // skip '.'
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                result.push_str(&expr[start..i]);
            } else {
                result.push_str(&expr[start..i]);
                result.push_str(".0");
            }
        } else {
            result.push(bytes[i] as char);
            i += 1;
        }
    }
    result
}

fn main() {
    let stdin = std::io::stdin();
    let query = match stdin.lock().lines().next() {
        Some(Ok(line)) => line.trim().to_string(),
        _ => return,
    };

    if query.is_empty() {
        return;
    }

    // Convert integer literals to floats to avoid integer division (1/3 → 0)
    let float_query = intlit_to_float(&query);
    let result = match evalexpr::eval_number(&float_query) {
        Ok(f) => f,
        Err(_) => return,
    };

    // Format: omit trailing ".0" for integers
    let formatted = if result.fract() == 0.0 && result.abs() < i64::MAX as f64 {
        format!("{}", result as i64)
    } else {
        format!("{result}")
    };

    // Don't show result if it's the same as the input (e.g. "42" → 42)
    if formatted == query {
        return;
    }

    let item = serde_json::json!({
        "title": format!("= {formatted}"),
        "subtitle": "Copy to clipboard",
        "data": formatted,
    });
    println!("{}", serde_json::to_string(&item).unwrap());
}
