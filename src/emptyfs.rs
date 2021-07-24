extern crate env_logger;
extern crate fuse;

use std::env;
use fuse::{Filesystem, Request};
use std::os::raw::c_int;

struct EmptyFS;

impl Filesystem for EmptyFS {
    fn init(&mut self, _req: &Request) -> Result<(), c_int> {
        println!("my init");
        Ok(())
    }

    fn destroy(&mut self, _req: &Request) {
        println!("my destroy");
    }
}

fn main() {
    env_logger::init();
    let mountpoint = env::args_os().nth(1).expect("usage: emptyfs MOUNTPOINT");
    fuse::mount(EmptyFS, &mountpoint, &[]).expect("fail mount()");
}
