extern crate time;

use std::io::{Read, Write, Seek};
use std::process::exit;

const HELP_TEXT: &'static str = "netcp (send,receive) ipaddress[:port] filename";
const CALL_SIGN: &'static str = "netcp v0.1";
const MSG_AGREE: &'static str = "AGREE   ";
const MSG_DISAGREE: &'static str = "DISAGREE";
const MSG_FILE: &'static str = "FILE";
const MSG_END: &'static str = "END ";
const TIMEOUT: i64 = 800;

fn main() {
    //get commandline arguments
    let args: Vec<String> = std::env::args().collect();

    //if there is no argument (first arg is always application name), error
    if args.len() == 1 {
        error("No arguments");
    }

    //execute option
    match args[1].as_ref() {
        "help" => {
            println!("{}", HELP_TEXT);
        }

        "send" => {
            //sending needs min. 4 args (appl. name, "send", address, filenames...)
            if args.len() < 4 {
                error("Too few arguments");
            }

            send(&args[2], &args[3..]);
        }

        "receive" => {
            //receiving needs exactly 3 args (appl. name, "receive", address)
            if args.len() != 3 {
                error("Too few or many arguments");
            }

            receive(&args[2]);
        }

        //Unknown parameter
        _ => {
            error("Unknown parameter");
        }
    }
}

fn error(msg: &str) {
    println!("Error: {}.", msg);
    std::process::exit(1);
}

fn receive_data(stream: &mut std::net::TcpStream, buf: &mut [u8]) {
    let mut begin = time::get_time();
    let mut received_bytes: usize = 0;

    while received_bytes < buf.len() && (begin - time::get_time()).num_milliseconds() < TIMEOUT {
        let res = stream.read(&mut buf[received_bytes..]);

        if res.is_err() {
            error("Connection lost");
        }

        let bytes = res.unwrap();
        received_bytes += bytes;

        if bytes != 0 {
            begin = time::get_time();
        }
    }

    if received_bytes < buf.len() {
        error("Connection lost (timeout)");
    }
}

fn send_data(stream: &mut std::net::TcpStream, data: &[u8]) {
    let mut begin = time::get_time();
    let mut sended_bytes: usize = 0;

    while sended_bytes < data.len() && (begin - time::get_time()).num_milliseconds() < TIMEOUT {
        let res = stream.write(&data[sended_bytes..]);

        if res.is_err() {
            error("Connection lost");
        }

        let bytes = res.unwrap();
        sended_bytes += bytes;

        if bytes != 0 {
            begin = time::get_time();
        }
    }

    if sended_bytes < data.len() {
        error("Connection lost (timeout)");
    }
}

fn send_u64(stream: &mut std::net::TcpStream, num: u64) {
    let data = num.to_le();
    let buf: &[u8] = unsafe { std::mem::transmute::<&u64, &[u8; 8]>(&data) };
    send_data(stream, &buf);
}

fn receive_u64(stream: &mut std::net::TcpStream) -> u64 {
    let mut num = 0u64;
    receive_data(stream, unsafe { std::mem::transmute::<&mut u64, &mut [u8; 8]>(&mut num) });
    return u64::from_le(num);
}

fn send_string(stream: &mut std::net::TcpStream, string: &str) {
    let size = string.len() as u64;
    send_u64(stream, size);
    send_data(stream, string.as_bytes());
}

fn receive_string(stream: &mut std::net::TcpStream) -> String {
    let size = receive_u64(stream);
    let mut vec: Vec<u8> = Vec::with_capacity(size as usize);
    unsafe { vec.set_len(size as usize); };
    receive_data(stream, &mut vec[..] );
    let string_res = String::from_utf8(vec);
    match string_res {
        Err(_) => { error("Couldn't convert bytes to string"); }
        _ => {}
    }
    return string_res.unwrap();
}

fn check_agreement(stream: &mut std::net::TcpStream) -> bool {
    let mut vec: Vec<u8> = Vec::with_capacity(MSG_AGREE.len());
    unsafe { vec.set_len(MSG_AGREE.len()); };
    receive_data(stream, &mut vec[..]);

    if compare_byte_array(&vec[..], MSG_AGREE.as_bytes()) {
        return true;
    }
    else if compare_byte_array(&vec[..], MSG_DISAGREE.as_bytes()) {
        return false;
    }
    else {
        error("Invalid protocol");
    }
    return false;
}

fn compare_byte_array(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    for i in 0..a.len() {
        if a[i] != b[i] {
            return false;
        }
    }

    return true;
}

fn get_filesize(file: &mut std::fs::File) -> u64 {
    let current = file.seek(std::io::SeekFrom::Current(0)).ok().expect("Error: file seeking failed.");
    let size = file.seek(std::io::SeekFrom::End(0)).ok().expect("Error: file seeking failed.");
    file.seek(std::io::SeekFrom::Start(current)).ok().expect("Error: file seeking failed.");
    return size;
}

fn send(address: &String, file_names: &[String]) {
    //1. check file accessibilities
    //2. check address
    //3. connect with client
    //4. send files sequential

    let mut buf = [0u8; 512];

    //1. check file existences
    //get working directory to find correct files
    let work_dir_res = std::env::current_dir();
    match work_dir_res {
        Err(_) => { error("Couldn't find working directory"); },
        _ => {}
    }
    let work_dir = work_dir_res.unwrap();

    //join working directory and file_names to get absolute file addresses in a vector
    let mut file_addrs: Vec<std::path::PathBuf> = Vec::with_capacity(file_names.len());
    for file_name in file_names {
        file_addrs.push(work_dir.join(file_name));
    }

    //open and close files to check accessibility
    for file_addr in &file_addrs {
        if let Err(_) = std::fs::File::open(&file_addr) {
            println!("Error: File doesn't exist or is not accessible ({}).", file_addr.display());
            exit(1);
        }
    }

    //2. check address and bind listener
    let listener_res = std::net::TcpListener::bind(&address[..]);
    match listener_res {
        Err(ref e) => { println!("Error: {}.", e); exit(1); },
        _ => {}
    }
    let listener = listener_res.unwrap();

    //3. connect with client
    print!("Waiting for client..."); let _ = std::io::stdout().flush();
    let stream_res = listener.accept();
    match stream_res {
        Err(ref e) => { println!("Error: {}.", e); exit(1); },
        _ => {}
    }
    let (mut stream, client_addr) = stream_res.unwrap();

    //3. check if client send correct CALL_SIGN
    receive_data(&mut stream, &mut buf[..CALL_SIGN.len()]);

    if compare_byte_array(CALL_SIGN.as_bytes(), &buf[..CALL_SIGN.len()]) == false {
        error("Invalid protocol");
    }

    send_data(&mut stream, MSG_AGREE.as_bytes());

    println!("connected with {}.", client_addr); let _ = std::io::stdout().flush();

    //send files
    for file_addr in &file_addrs {

        let file_res = std::fs::File::open(&file_addr);
        match file_res {
            Err(_) => { error("Couldn't open file") },
            _ => {}
        }
        let mut file = file_res.unwrap();
        let filesize = get_filesize(&mut file);

        send_data(&mut stream, MSG_FILE.as_bytes()); //send file is ready to send
        send_u64(&mut stream, filesize); //send file size as u64

        //send filename
        let filename_os_opt = file_addr.file_name();
        if filename_os_opt.is_none() {
            error("Couldn't convert filename to utf8");
        }
        let filename_opt = filename_os_opt.unwrap().to_str();
        if filename_opt.is_none() {
            error("Couldn't convert filename to utf8");
        }
        send_string(&mut stream, &filename_opt.unwrap());

        print!("Send {}...", &filename_opt.unwrap()); let _ = std::io::stdout().flush();

        //if client send MSG_DISAGREE, continue else send file
        if check_agreement(&mut stream) == false {
            println!("cancelled by client.");
            continue;
        }

        //send file
        let mut i = 0u64;
        while i < (filesize-1) {
            if (filesize - i) >= buf.len() as u64 {
                if let Err(_) = file.read(&mut buf) {
                    error("Couldn't read from file");
                }
                send_data(&mut stream, &buf);
                i += buf.len() as u64;
            }
            else {
                if let Err(_) = file.read(&mut buf[..(filesize - i) as usize]) {
                    error("Couldn't read from file");
                }
                send_data(&mut stream, &buf[..(filesize - i) as usize]);
                i = filesize-1;
            }
        }

        println!("done."); let _ = std::io::stdout().flush();
    }

    //send end
    send_data(&mut stream, MSG_END.as_bytes());
}

fn receive(address: &String) {
    let mut buf = [0u8; 512];

    let work_dir_res = std::env::current_dir();
    match work_dir_res {
        Err(_) => { error("Couldn't find working directory"); },
        _ => {}
    }
    let work_dir = work_dir_res.unwrap();

    let stream_res = std::net::TcpStream::connect(&address[..]);
    match stream_res {
        Err(ref e) => { println!("Error: {}.", e); exit(1); },
        _ => {}
    }
    let mut stream = stream_res.unwrap();

    send_data(&mut stream, CALL_SIGN.as_bytes());

    if check_agreement(&mut stream) == false {
        error("No server found");
    }

    let mut msg_file: Vec<u8> = Vec::with_capacity(MSG_FILE.len());
    unsafe { msg_file.set_len(MSG_FILE.len()); };
    loop {
        receive_data(&mut stream, &mut msg_file[..]);

        if compare_byte_array(&msg_file[..], MSG_END.as_bytes()) {
            break;
        }
        else if compare_byte_array(&msg_file[..], MSG_FILE.as_bytes()) == false {
            error("Invalid protocol");
        }

        let filesize = receive_u64(&mut stream);
        let filename = receive_string(&mut stream);

        let file_res = std::fs::File::create(work_dir.join(&filename));
        match file_res {
            Err(_) => {
                println!("Couldn't create file {}. Skip file transmition.", filename);
                send_data(&mut stream, MSG_DISAGREE.as_bytes());
                continue;
            },
            _ => {}
        }
        let mut file = file_res.unwrap();

        send_data(&mut stream, MSG_AGREE.as_bytes());

        print!("Receive {}...", filename); let _ = std::io::stdout().flush();

        //get file data
        let mut i = 0u64;
        while i < (filesize-1) {
            if (filesize - i) >= buf.len() as u64 {
                receive_data(&mut stream, &mut buf);
                if let Err(_) = file.write(&buf) {
                    error("Couldn't write to file");
                }
                i += buf.len() as u64;
            }
            else {
                receive_data(&mut stream, &mut buf[..(filesize - i) as usize]);
                if let Err(_) = file.write(&buf[..(filesize - i) as usize]) {
                    error("Couldn't write to file");
                }
                i = filesize-1;
            }
        }

        println!("done."); let _ = std::io::stdout().flush();
    }
}
