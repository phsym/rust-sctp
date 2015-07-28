extern crate sctp;
use sctp::*;

fn main() {
    println!("Hello, world!");
    
//    match SctpStream::connect("127.0.0.1:3868") {
//    	Err(e) => println!("{:?}", e.kind()),
//    	_ => println!("OK")
//    }

//	match SctpListener::bind("0.0.0.0:3868") {
//	match SctpListener::bindx(&["10.0.2.15:3868", "127.0.0.1:3868"]) {
//		Ok(serv) => {		
//			println!("bound to {:?}", serv.local_addrs().unwrap());
//			match serv.accept() {
//				Err(e) => println!("{:?}", e.kind()),
//				Ok((peer, _)) => {
//					let p2 = peer.try_clone().unwrap();
//					println!("connection from {:?} on {:?}", p2.peer_addrs().unwrap(), p2.local_addrs().unwrap());
//					p2.sendmsg(6, "taatayoyooo\n".as_bytes()).unwrap();
//					let mut reply = [0u8; 1024];
//					let (len, stream) = p2.recvmsg(&mut reply).unwrap();
//					println!("Received {} bytes on {}", len, stream);
//				}
//			};
//		},
//		Err(e) => panic!("{:?}", e.kind())
//	}

//	let sock = match SctpDatagram::bindx(&["10.0.2.15:3868", "127.0.0.1:3868"]) {
	let sock = match SctpDatagram::bind("0.0.0.0:3868") {
		Ok(s) => s,
		Err(e) => panic!("{:?}", e.kind())
	};
	println!("Bound to {:?}", sock.local_addrs().unwrap());
	let mut buf = [0u8; 1024];
	match sock.recv_from(&mut buf) {
		Ok((len, stream, addr)) => println!("Received {} bytes from {} on stream {}", len, addr, stream),
		Err(e) => println!("{:?}", e.kind())
	};
}
