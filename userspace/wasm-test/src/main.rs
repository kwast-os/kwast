use std::env;
use std::fs::File;
use std::io::Write;

fn main() {
    // File::create("myfile").expect("lol");

    /*println!("abc");

    for (k, v) in env::vars() {
        println!("{}: {}", k, v);
    }

    println!("-----");*/

    let mut test = File::open(".").expect("open test");
    test.write(b"abc").expect("write test");
}
