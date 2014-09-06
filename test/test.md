# Hello World

```rust
extern crate md_rel;
use std::os::args;

fn main() {
    for file in args().iter() {
        let file = file.as_slice();
        md_rel::transform_file(file);
    }
}

```

# Goodbye World
