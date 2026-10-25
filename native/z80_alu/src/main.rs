use std::error::Error;
use std::io::{self, BufRead};

use sega_z80_alu::{add8, sub8};

fn main() -> Result<(), Box<dyn Error>> {
    for line in io::stdin().lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let mut fields = line.split_whitespace();
        let operation = fields.next().ok_or("missing operation")?;
        let lhs = u8::from_str_radix(fields.next().ok_or("missing lhs")?, 16)?;
        let rhs = u8::from_str_radix(fields.next().ok_or("missing rhs")?, 16)?;
        if fields.next().is_some() {
            return Err("too many input fields".into());
        }

        let result = match operation {
            "A" => add8(lhs, rhs),
            "S" => sub8(lhs, rhs),
            _ => return Err(format!("unsupported operation: {operation}").into()),
        };
        println!("{:02X} {:02X}", result.value, result.flags);
    }

    Ok(())
}
