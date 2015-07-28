extern crate sctp;
use sctp::*;

fn main() {
//	match SctpListener::bind("0.0.0.0:3868") {
	match SctpListener::bindx(&["10.0.2.15:3868", "127.0.0.1:3868"]) {
		Ok(serv) => {		
			println!("bound to {:?}", serv.local_addrs().unwrap());
			match serv.accept() {
				Err(e) => println!("{:?}", e.kind()),
				Ok((peer, _)) => {
					println!("connection from {:?} on {:?}", peer.peer_addrs().unwrap(), peer.local_addrs().unwrap());
					// Send message on stream 6
					peer.sendmsg("foobar\n".as_bytes(), 6).unwrap();
					let mut reply = [0u8; 1024];
					let (len, stream) = peer.recvmsg(&mut reply).unwrap();
					println!("Received {} bytes on stream {}", len, stream);
				}
			};
		},
		Err(e) => panic!("{:?}", e.kind())
	}
}
