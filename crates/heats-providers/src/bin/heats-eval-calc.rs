use std::io::BufRead;

fn main() {
    let stdin = std::io::stdin();
    let query = match stdin.lock().lines().next() {
        Some(Ok(line)) => line.trim().to_string(),
        _ => return,
    };

    if query.is_empty() {
        return;
    }

    let result = match meval::eval_str(&query) {
        Ok(v) => v,
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
