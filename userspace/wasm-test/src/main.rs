use std::fs::File;

fn main() {
    File::create("myfile").expect("lol");
}
