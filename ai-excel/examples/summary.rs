//! Dump a compact JSON summary of a workbook parsed by json2xlsx.
//! Usage: cargo run --example summary -- file.xlsx

use json2xlsx::model::Workbook;
use serde_json::{json, Map, Value};

fn main() {
    let path = match std::env::args().nth(1) {
        Some(p) => p,
        None => {
            eprintln!("usage: summary <file.xlsx>");
            std::process::exit(1);
        }
    };
    let wb: Workbook = match json2xlsx::parse(std::path::Path::new(&path)) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("ERR {e}");
            std::process::exit(2);
        }
    };

    let sheets: Vec<Value> = wb
        .sheets
        .iter()
        .map(|s| {
            let mut cells = Map::new();
            for row in &s.rows {
                for c in &row.cells {
                    let Some(reference) = &c.reference else {
                        continue;
                    };
                    let v = if let Some(f) = &c.formula {
                        json!({ "f": f })
                    } else {
                        match &c.value {
                            Some(Value::Number(n)) => json!({ "n": n.as_f64() }),
                            Some(Value::String(t)) => json!({ "s": t }),
                            Some(Value::Bool(b)) => json!({ "b": b }),
                            _ => json!({ "e": true }),
                        }
                    };
                    cells.insert(reference.clone(), v);
                }
            }
            json!({ "name": s.name, "cells": cells })
        })
        .collect();

    println!("{}", json!({ "sheets": sheets }));
}
