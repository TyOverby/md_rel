#![feature(core)]
#![feature(os)]

extern crate md_rel;
use std::os::args;

fn main() {
    for file in args().iter() {
        let file = file.as_slice();
        let _ = md_rel::transform_file(file);
    }
}
