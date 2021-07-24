extern crate env_logger;
extern crate fuse;
extern crate libc;
extern crate time;

use fuse::{
    FileAttr, FileType, Filesystem, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory, ReplyEntry,
    ReplyWrite, Request,
};
use libc::{EACCES, ENOENT};
use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;
use time::Timespec;

const TTL: Timespec = Timespec { sec: 1, nsec: 0 };

type INode = u64;

struct File {
    parent_ino: INode,
    name: String,
    attr: FileAttr,
    generation: u64,
}

struct MemFS {
    inodes: HashMap<INode, File>,  // <ino, File>
    datas: HashMap<INode, String>, // <ino, file_data>
}

fn new_file_attr(ino: INode, size: u64, ftype: FileType, uid: u32, gid: u32) -> FileAttr {
    let t = time::now().to_timespec();
    FileAttr {
        ino: ino,
        size: size,
        blocks: 0,
        atime: t,
        mtime: t,
        ctime: t,
        crtime: t,
        kind: ftype,
        perm: match ftype {
            FileType::Directory => 0o755,
            _ => 0o644,
        },
        nlink: match ftype {
            FileType::Directory => 2,
            _ => 1,
        },
        uid: uid,
        gid: gid,
        rdev: 0,
        flags: 0,
    }
}

fn find_child_file(m: &MemFS, parent_ino: INode, name: &str) -> Option<INode> {
    for (ino, f) in m.inodes.iter() {
        if f.parent_ino == parent_ino && f.name == name {
            return Some(ino.clone());
        }
    }
    None
}

impl Filesystem for MemFS {
    fn getattr(&mut self, _req: &Request, ino: INode, reply: ReplyAttr) {
        for (&inode, f) in self.inodes.iter() {
            if ino == inode {
                reply.attr(&TTL, &f.attr);
                return;
            }
        }
        reply.error(ENOENT);
    }

    fn readdir(
        &mut self,
        _req: &Request,
        ino: INode,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        if offset > 0 {
            reply.ok();
            return;
        }

        reply.add(1, 0, FileType::Directory, ".");
        reply.add(2, 1, FileType::Directory, "..");
        let mut reply_add_offset = 2;
        for (_, f) in self.inodes.iter() {
            if ino == f.parent_ino {
                let attr = f.attr;
                let name = &f.name;
                reply.add(attr.ino, reply_add_offset, attr.kind, name);
                reply_add_offset += 1;
            }
        }
        reply.ok();
    }

    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        for (_, f) in self.inodes.iter() {
            if f.parent_ino == parent && name.to_str().unwrap() == f.name.as_str() {
                reply.entry(&TTL, &f.attr, 0);
                return;
            }
        }
        reply.error(ENOENT);
    }

    fn create(
        &mut self,
        _req: &Request,
        parent: u64,
        name: &OsStr,
        _mode: u32,
        _flag: u32,
        reply: ReplyCreate,
    ) {
        let inode = time::now().to_timespec().sec as u64;
        let f = new_file_attr(inode, 0, FileType::RegularFile, _req.uid(), _req.gid());
        self.inodes.insert(
            inode,
            File {
                parent_ino: parent,
                name: name.to_str().unwrap().to_string(),
                attr: f,
                generation: 0,
            },
        );
        reply.created(&TTL, &f, 0, 0, 0);
    }

    fn mkdir(
        &mut self,
        _req: &Request,
        _parent: u64,
        _name: &OsStr,
        _mode: u32,
        reply: ReplyEntry,
    ) {
        let inode = time::now().to_timespec().sec as u64;
        let f = new_file_attr(inode, 0, FileType::Directory, _req.uid(), _req.gid());
        self.inodes.insert(
            inode,
            File {
                parent_ino: _parent,
                name: _name.to_str().unwrap().to_string(),
                attr: f,
                generation: 0,
            },
        );
        reply.entry(&TTL, &f, 0);
    }

    fn setattr(
        &mut self,
        _req: &Request,
        ino: INode,
        _mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        _size: Option<u64>,
        _atime: Option<Timespec>,
        _mtime: Option<Timespec>,
        _fh: Option<u64>,
        _crtime: Option<Timespec>,
        _chgtime: Option<Timespec>,
        _bkuptime: Option<Timespec>,
        _flags: Option<u32>,
        reply: ReplyAttr,
    ) {
        match self.inodes.get_mut(&ino) {
            Some(mut f) => {
                f.attr.size = _size.unwrap_or(f.attr.size);
                f.attr.uid = _uid.unwrap_or(f.attr.uid);
                f.attr.gid = _gid.unwrap_or(f.attr.gid);
                f.attr.mtime = _mtime.unwrap_or(f.attr.mtime);
                f.attr.flags = _flags.unwrap_or(f.attr.flags);
                f.generation += 1;
                reply.attr(&TTL, &f.attr)
            }
            None => reply.error(EACCES),
        }
    }

    fn write(
        &mut self,
        _req: &Request,
        ino: INode,
        _fh: u64,
        _offset: i64,
        data: &[u8],
        _flags: u32,
        reply: ReplyWrite,
    ) {
        let length: usize = data.len();
        let x = String::from_utf8(data.to_vec()).expect("fail to-string");
        self.datas.insert(ino, x);
        if let Some(f) = self.inodes.get_mut(&ino) {
            let parent_ino = f.parent_ino;
            let name = f.name.clone();
            *f = File {
                parent_ino,
                name,
                attr: new_file_attr(
                    ino,
                    length as u64,
                    FileType::RegularFile,
                    _req.uid(),
                    _req.gid(),
                ),
                generation: f.generation + 1,
            }
        }
        reply.written(length as u32);
    }

    fn read(
        &mut self,
        _req: &Request,
        ino: INode,
        _fh: u64,
        _offset: i64,
        _size: u32,
        reply: ReplyData,
    ) {
        match self.datas.get(&ino) {
            Some(x) => reply.data(x.as_bytes()),
            None => reply.error(EACCES),
        }
    }

    fn unlink(&mut self, _req: &Request, _parent: u64, _name: &OsStr, reply: fuse::ReplyEmpty) {
        let maybe_ino = find_child_file(self, _parent, _name.to_str().unwrap());
        match maybe_ino {
            Some(ino) => {
                self.inodes.remove(&ino);
                self.datas.remove(&ino);
                reply.ok();
            }
            None => reply.error(EACCES),
        }
    }

    fn rmdir(&mut self, _req: &Request, _parent: u64, _name: &OsStr, reply: fuse::ReplyEmpty) {
        self.unlink(_req, _parent, _name, reply)
    }
}

fn main() {
    env_logger::init();
    let mountpoint = env::args_os().nth(1).expect("usage: backlogfs MOUNTPOINT");
    let mut inodes = HashMap::new();
    let datas = HashMap::new();
    // i-node numberの1はroot node, 0はbad block
    inodes.insert(
        1,
        File {
            parent_ino: 0,
            name: "/".to_string(),
            attr: new_file_attr(1, 0, FileType::Directory, 501, 20),
            generation: 0,
        },
    );
    fuse::mount(
        MemFS {
            inodes: inodes,
            datas: datas,
        },
        &mountpoint,
        &[],
    )
    .expect("fail mount()");
}
