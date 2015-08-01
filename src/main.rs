extern crate md_rel;
use std::env::args;

fn main() {
    for file in args() {
        let _ = md_rel::transform_file(&file);
    }
}
