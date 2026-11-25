//! The `tela` command.
#![forbid(unsafe_code)]

fn main() {
    println!("tela {}", env!("CARGO_PKG_VERSION"));
}
