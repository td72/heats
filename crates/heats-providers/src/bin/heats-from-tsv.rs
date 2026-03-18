use heats_core::source::DmenuItem;
use std::io::{self, BufRead};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let opts = match parse_args(&args[1..]) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("heats-from-tsv: {e}");
            std::process::exit(1);
        }
    };

    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();

    // Skip header if requested
    if opts.header {
        let _ = lines.next();
    }

    for line in lines {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        if line.is_empty() {
            continue;
        }

        let cols: Vec<&str> = if opts.collapse {
            split_collapse(&line, opts.delimiter, opts.max_col)
        } else {
            line.split(opts.delimiter).collect()
        };

        let title = match get_col(&cols, opts.title_col) {
            Some(s) => s.to_string(),
            None => continue,
        };

        let subtitle = if opts.subtitle_cols.is_empty() {
            None
        } else {
            let parts: Vec<&str> = opts
                .subtitle_cols
                .iter()
                .filter_map(|&c| get_col(&cols, c))
                .filter(|s| !s.is_empty())
                .collect();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join(" · "))
            }
        };

        let data = if opts.data_fields.is_empty() {
            None
        } else {
            let mut map = serde_json::Map::new();
            for &(ref key, col) in &opts.data_fields {
                if let Some(val) = get_col(&cols, col) {
                    map.insert(key.clone(), serde_json::Value::String(val.to_string()));
                }
            }
            if map.is_empty() {
                None
            } else {
                Some(serde_json::Value::Object(map))
            }
        };

        let item = DmenuItem {
            title,
            subtitle,
            icon_path: None,
            data,
        };
        println!("{}", serde_json::to_string(&item).unwrap());
    }
}

/// Split a line by delimiter, collapsing consecutive delimiters.
/// Once `max_col - 1` columns have been collected, the remainder of the line
/// (with leading delimiters stripped) becomes the last column.
fn split_collapse(line: &str, delimiter: char, max_col: usize) -> Vec<&str> {
    let mut cols = Vec::new();
    let mut rest = line;
    while !rest.is_empty() {
        // Skip leading delimiters
        rest = rest.trim_start_matches(delimiter);
        if rest.is_empty() {
            break;
        }
        // If we've collected max_col - 1 columns, take the rest as-is
        if cols.len() + 1 >= max_col {
            cols.push(rest);
            break;
        }
        // Take next token
        match rest.find(delimiter) {
            Some(pos) => {
                cols.push(&rest[..pos]);
                rest = &rest[pos..];
            }
            None => {
                cols.push(rest);
                break;
            }
        }
    }
    cols
}

fn get_col<'a>(cols: &[&'a str], index: usize) -> Option<&'a str> {
    if index == 0 {
        return None;
    }
    cols.get(index - 1).copied()
}

struct Opts {
    title_col: usize,
    subtitle_cols: Vec<usize>,
    data_fields: Vec<(String, usize)>,
    delimiter: char,
    header: bool,
    collapse: bool,
    /// Maximum column number referenced (used with --collapse to join remainder into last column)
    max_col: usize,
}

fn parse_args(args: &[String]) -> Result<Opts, String> {
    let mut title_col: Option<usize> = None;
    let mut subtitle_cols: Vec<usize> = Vec::new();
    let mut data_fields: Vec<(String, usize)> = Vec::new();
    let mut delimiter = '\t';
    let mut header = false;
    let mut collapse = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--title" => {
                i += 1;
                title_col = Some(parse_col(args, i, "--title")?);
            }
            "--subtitle" => {
                i += 1;
                let val = arg_value(args, i, "--subtitle")?;
                subtitle_cols = val
                    .split(',')
                    .map(|s| {
                        s.trim()
                            .parse::<usize>()
                            .map_err(|_| format!("invalid column number: '{s}'"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
            }
            "--data-field" => {
                i += 1;
                let val = arg_value(args, i, "--data-field")?;
                let (key, col_str) = val
                    .split_once('=')
                    .ok_or_else(|| format!("--data-field must be key=col, got: '{val}'"))?;
                let col: usize = col_str
                    .parse()
                    .map_err(|_| format!("invalid column number: '{col_str}'"))?;
                data_fields.push((key.to_string(), col));
            }
            "--delimiter" => {
                i += 1;
                let val = arg_value(args, i, "--delimiter")?;
                delimiter = val
                    .chars()
                    .next()
                    .ok_or_else(|| "--delimiter value is empty".to_string())?;
            }
            "--header" => {
                header = true;
            }
            "--collapse" => {
                collapse = true;
            }
            other => {
                return Err(format!("unknown option: '{other}'"));
            }
        }
        i += 1;
    }

    let title_col = title_col.ok_or("--title is required")?;

    let max_col = [title_col]
        .iter()
        .copied()
        .chain(subtitle_cols.iter().copied())
        .chain(data_fields.iter().map(|(_, c)| *c))
        .max()
        .unwrap_or(1);

    Ok(Opts {
        title_col,
        subtitle_cols,
        data_fields,
        delimiter,
        header,
        collapse,
        max_col,
    })
}

fn arg_value<'a>(args: &'a [String], i: usize, name: &str) -> Result<&'a str, String> {
    args.get(i)
        .map(|s| s.as_str())
        .ok_or_else(|| format!("{name} requires a value"))
}

fn parse_col(args: &[String], i: usize, name: &str) -> Result<usize, String> {
    let val = arg_value(args, i, name)?;
    val.parse::<usize>()
        .map_err(|_| format!("invalid column number for {name}: '{val}'"))
}
