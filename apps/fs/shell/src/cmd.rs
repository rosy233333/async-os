use std::fs::{self, File, FileType};
use std::io;
use std::prelude::*;

use alloc::string::String;
use alloc::vec::Vec;

macro_rules! print_err {
    ($cmd: literal, $msg: expr) => {
        println!("{}: {}", $cmd, $msg);
    };
    ($cmd: literal, $arg: expr, $err: expr) => {
        println!("{}: {}: {}", $cmd, $arg, $err);
    };
}

const CMD_TABLE: &[&str] = &[
    "cat", "cd", "echo", "help", "ls", "mkdir", "pwd", "rm", "uname",
];

fn file_type_to_char(ty: FileType) -> char {
    if ty.is_char_device() {
        'c'
    } else if ty.is_block_device() {
        'b'
    } else if ty.is_socket() {
        's'
    } else if ty.is_fifo() {
        'p'
    } else if ty.is_symlink() {
        'l'
    } else if ty.is_dir() {
        'd'
    } else if ty.is_file() {
        '-'
    } else {
        '?'
    }
}

#[rustfmt::skip]
const fn file_perm_to_rwx(mode: u32) -> [u8; 9] {
    let mut perm = [b'-'; 9];
    macro_rules! set {
        ($bit:literal, $rwx:literal) => {
            if mode & (1 << $bit) != 0 {
                perm[8 - $bit] = $rwx
            }
        };
    }

    set!(2, b'r'); set!(1, b'w'); set!(0, b'x');
    set!(5, b'r'); set!(4, b'w'); set!(3, b'x');
    set!(8, b'r'); set!(7, b'w'); set!(6, b'x');
    perm
}

async fn do_ls(args: &str) {
    let current_dir = std::env::current_dir().await.unwrap();
    let args = if args.is_empty() {
        path_to_str!(current_dir)
    } else {
        args
    };
    let name_count = args.split_whitespace().count();

    async fn show_entry_info(path: &str, entry: &str) -> io::Result<()> {
        let metadata = fs::metadata(path).await?;
        let size = metadata.len();
        let file_type = metadata.file_type();
        let file_type_char = file_type_to_char(file_type);
        let rwx = file_perm_to_rwx(metadata.permissions().mode());
        let rwx = unsafe { core::str::from_utf8_unchecked(&rwx) };
        println!("{}{} {:>8} {}", file_type_char, rwx, size, entry);
        Ok(())
    }

    async fn list_one(name: &str, print_name: bool) -> io::Result<()> {
        let is_dir = fs::metadata(name).await?.is_dir();
        if !is_dir {
            return show_entry_info(name, name).await;
        }

        if print_name {
            println!("{}:", name);
        }
        let mut entries = Vec::new();
        fs::read_dir(name)
            .await?
            .filter_map(|e| e.ok())
            .map(|e| e.file_name())
            .for_each(|e| {
                entries.push(e);
            })
            .await;
        entries.sort();

        for entry in entries {
            let entry = path_to_str!(entry);
            let path = String::from(name) + "/" + entry;
            if let Err(e) = show_entry_info(&path, entry).await {
                print_err!("ls", path, e);
            }
        }
        Ok(())
    }

    for (i, name) in args.split_whitespace().enumerate() {
        if i > 0 {
            println!();
        }
        if let Err(e) = list_one(name, name_count > 1).await {
            print_err!("ls", name, e);
        }
    }
}

async fn do_cat(args: &str) {
    if args.is_empty() {
        print_err!("cat", "no file specified");
        return;
    }

    async fn cat_one(fname: &str) -> io::Result<()> {
        let mut buf = [0; 1024];
        let mut file = File::open(fname).await?;
        loop {
            let n = file.read(&mut buf).await?;
            if n > 0 {
                io::stdout().write_all(&buf[..n]).await?;
            } else {
                return Ok(());
            }
        }
    }

    for fname in args.split_whitespace() {
        if let Err(e) = cat_one(fname).await {
            print_err!("cat", fname, e);
        }
    }
}

async fn do_echo(args: &str) {
    async fn echo_file(fname: &str, text_list: &[&str]) -> io::Result<()> {
        let mut file = File::create(fname).await?;
        for text in text_list {
            file.write_all(text.as_bytes()).await?;
        }
        Ok(())
    }

    if let Some(pos) = args.rfind('>') {
        let text_before = args[..pos].trim();
        let (fname, text_after) = split_whitespace(&args[pos + 1..]);
        if fname.is_empty() {
            print_err!("echo", "no file specified");
            return;
        };

        let text_list = [
            text_before,
            if !text_after.is_empty() { " " } else { "" },
            text_after,
            "\n",
        ];
        if let Err(e) = echo_file(fname, &text_list).await {
            print_err!("echo", fname, e);
        }
    } else {
        println!("{}", args)
    }
}

async fn do_mkdir(args: &str) {
    if args.is_empty() {
        print_err!("mkdir", "missing operand");
        return;
    }

    async fn mkdir_one(path: &str) -> io::Result<()> {
        fs::create_dir(path).await
    }

    for path in args.split_whitespace() {
        if let Err(e) = mkdir_one(path).await {
            print_err!("mkdir", format_args!("cannot create directory '{path}'"), e);
        }
    }
}

async fn do_rm(args: &str) {
    if args.is_empty() {
        print_err!("rm", "missing operand");
        return;
    }
    let mut rm_dir = false;
    for arg in args.split_whitespace() {
        if arg == "-d" {
            rm_dir = true;
        }
    }

    async fn rm_one(path: &str, rm_dir: bool) -> io::Result<()> {
        if rm_dir && fs::metadata(path).await?.is_dir() {
            fs::remove_dir(path).await
        } else {
            fs::remove_file(path).await
        }
    }

    for path in args.split_whitespace() {
        if path == "-d" {
            continue;
        }
        if let Err(e) = rm_one(path, rm_dir).await {
            print_err!("rm", format_args!("cannot remove '{path}'"), e);
        }
    }
}

async fn do_cd(mut args: &str) {
    if args.is_empty() {
        args = "/";
    }
    if !args.contains(char::is_whitespace) {
        if let Err(e) = std::env::set_current_dir(args).await {
            print_err!("cd", args, e);
        }
    } else {
        print_err!("cd", "too many arguments");
    }
}

async fn do_pwd(_args: &str) {
    let pwd = std::env::current_dir().await.unwrap();
    println!("{}", path_to_str!(pwd));
}

async fn do_uname(_args: &str) {
    let arch = option_env!("AX_ARCH").unwrap_or("");
    let platform = option_env!("AX_PLATFORM").unwrap_or("");
    let smp = match option_env!("AX_SMP") {
        None | Some("1") => "",
        _ => " SMP",
    };
    let version = option_env!("CARGO_PKG_VERSION").unwrap_or("0.1.0");
    println!(
        "ArceOS {ver}{smp} {arch} {plat}",
        ver = version,
        smp = smp,
        arch = arch,
        plat = platform,
    );
}

async fn do_help(_args: &str) {
    println!("Available commands:");
    for name in CMD_TABLE {
        println!("  {}", name);
    }
}

pub async fn run_cmd(line: &[u8]) {
    let line_str = unsafe { core::str::from_utf8_unchecked(line) };
    let (cmd, args) = split_whitespace(line_str);
    if !cmd.is_empty() {
        match cmd {
            "cat" => do_cat(args).await,
            "cd" => do_cd(args).await,
            "echo" => do_echo(args).await,
            "help" => do_help(args).await,
            "ls" => do_ls(args).await,
            "mkdir" => do_mkdir(args).await,
            "pwd" => do_pwd(args).await,
            "rm" => do_rm(args).await,
            "uname" => do_uname(args).await,
            _ => println!("{}: command not found", cmd),
        }
    }
}

fn split_whitespace(str: &str) -> (&str, &str) {
    let str = str.trim();
    str.find(char::is_whitespace)
        .map_or((str, ""), |n| (&str[..n], str[n + 1..].trim()))
}
