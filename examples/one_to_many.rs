extern crate sctp;
use sctp::*;

fn main() {
	// Create a new Sctp endpoint, and bind it to one or more socket addresses
//	let sock = match SctpEndpoint::bind("0.0.0.0:3868") {
	let sock = match SctpEndpoint::bindx(&["10.0.2.15:3868", "127.0.0.1:3868"]) {
		Ok(s) => s,
		Err(e) => panic!("{:?}", e.kind())
	};
	println!("Bound to {:?}", sock.local_addrs().unwrap());
	
	let mut buf = [0u8; 1024];
	
	// Read a message
	match sock.recv_from(&mut buf) {
		Ok((len, stream, addr)) => println!("Received {} bytes from {} on stream {} from {}", len, addr, stream, addr),
		Err(e) => println!("{:?}", e.kind())
	};
	
	sock.send_to(&mut buf, "191.168.1.2:3868", 6).unwrap();
}
