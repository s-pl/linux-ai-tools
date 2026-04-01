use std::io::{self, Read};

use ai_linux_tools::estimate_tokens_realistic;

fn main() {
    let mut input = String::new();
    if io::stdin().read_to_string(&mut input).is_err() {
        println!("0");
        return;
    }
    println!("{}", estimate_tokens_realistic(&input));
}
