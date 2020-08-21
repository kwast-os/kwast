use std::env;
use std::fs::File;
use std::io::Write;
use std::io::Read;

#[derive(Debug, Copy, Clone)]
#[repr(transparent)]
pub struct FileHandle(u64);

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub enum CommandData {
    Open(i32),
    Read(u64),
}

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct Command {
    sender: u64,
    payload: CommandData,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct ReplyPayload {
    status: u16,
    value: u64,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Reply {
    to: u64,
    payload: ReplyPayload,
}

fn main() {
    // File::create("myfile").expect("lol");

    /*println!("abc");

    for (k, v) in env::vars() {
        println!("{}: {}", k, v);
    }

    println!("-----");*/

    println!("Hello");

    let mut file = File::open(".").expect("open test");
    let mut buffer = [0u8; 64];
    for i in 0..10000 {
        let res = file.read(&mut buffer[..]).expect("read test");
        //println!("{}", res);
        let test = unsafe {
            std::slice::from_raw_parts(&buffer[..] as *const _ as *const Command, 1) // TODO
        };

        let command = test[0];
        //println!("read one: {:?}", command);
        //assert_eq!(command.sender, 1);

        let mut test = unsafe {
            std::slice::from_raw_parts_mut(&mut buffer[..] as *mut _ as *mut Reply, 1) // TODO
        };
        test[0] = Reply {
            to: command.sender,
            payload: ReplyPayload {
                status: 0,
                value: 12,
            },
        };

        let res = file.write(&mut buffer[..24]).expect("write test");
        //println!("{} {} w", res, i-1);
    }
    println!("end");
}
