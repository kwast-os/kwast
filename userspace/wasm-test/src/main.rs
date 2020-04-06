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

    // Bad way to calculate PI, but just as a test for floating point
    let mut nom = 1.0;
    let mut denom = 1.0;
    let mut res = 0.0;
    loop {
        println!("{} {}", 4.0 * res, denom);
        res += nom / denom;
        denom += 2.0;
        nom *= -1.0;
    }
}
