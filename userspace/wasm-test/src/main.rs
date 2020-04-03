fn other_function(n: i32) -> (i32, i32) {
    let mut vec = Vec::new();
    for i in 0..n {
        vec.push(i);
    }

    (vec.iter().sum::<i32>(), (n - 1) * n / 2)
}

fn main() {
    let n = 10000;
    println!("Hello, world! This is a Rust program compiled with the wasm32-wasi toolchain.");
    println!("The .wasm file (this program) was put in an initrd and this is now executing.");
    println!("{}: {:?}", n, other_function(n));
}
