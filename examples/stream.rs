extern crate sctp;
use sctp::*;

use std::io::prelude::*;

fn main() {
	// Create a new one-to-one stream
//    match SctpStream::connect("127.0.0.1:3868") {
	match SctpStream::connectx(&["10.0.2.15:3868", "127.0.0.1:3868"]) {
    	Err(e) => println!("{:?}", e.kind()),
    	Ok(mut peer) => {
    		// Set SCTP no delay
    		println!("{}", peer.has_nodelay().unwrap());
    		peer.set_nodelay(true).unwrap();
    		println!("{}", peer.has_nodelay().unwrap());
    		
    		// Set socket send buffer size
    		let oldsize = peer.get_buffer_size(SoDirection::Send).unwrap();
    		peer.set_buffer_size(SoDirection::Send, 4096).unwrap();
    		println!("Set send buffer size to {} (was : {})", peer.get_buffer_size(SoDirection::Send).unwrap(), oldsize);
    		
    		println!("Setting read timeout to 10 s");
    		peer.set_timeout(SoDirection::Receive, 10).unwrap();
    		
    		// Write a message using the io::Write trait
    		peer.write_all("foo bar\n".as_bytes()).unwrap();
    		// Write a message on stream 6
    		peer.sendmsg("foo bar again\n".as_bytes(), 6).unwrap();
    		let mut data = [0u8; 1024];
    		// Read data using the io::Read trait
    		peer.read(&mut data).unwrap();
    		// Read data using SCTP advanced feature, and retrieve the stream id
    		// on which data were received
    		let (size, stream) = peer.recvmsg(&mut data).unwrap();
    		println!("Received {} bytes on stream {}", size, stream);
		}
    }
}
