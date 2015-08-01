//! This crate provides high level SCTP networking.
//! Currently it only supports basic SCTP features like multi-homing
//! in one-to-one and one-to-many associations.
//! SCTP notifications and working directly on associtaion is not supported yet
//! but is in the TODO list.  

extern crate sctp_sys;
extern crate libc;

mod sctpsock;
use sctpsock::{SctpSocket, BindOp, RawSocketAddr};
use sctp_sys::{SOCK_SEQPACKET, SOL_SCTP};

use std::io::prelude::*;
use std::io::{Result, Error, ErrorKind};
use std::net::{ToSocketAddrs, SocketAddr, Shutdown};

#[cfg(target_os="linux")]
use std::os::unix::io::{AsRawFd, RawFd, FromRawFd};
#[cfg(target_os="windows")]
use std::os::windows::io::{AsRawHandle, RawHandle, FromRawHandle};

/// Socket buffer type
pub enum SoBuffer {
	/// RCV buffer
	Receive,
	/// SND buffer
	Send
}

impl SoBuffer {
	fn optname(&self) -> libc::c_int {
		return match *self {
			SoBuffer::Receive => libc::SO_RCVBUF,
			SoBuffer::Send => libc::SO_SNDBUF
		};
	}
}

/// One-to-one SCTP connected stream which behaves like a TCP stream.
/// A `SctpStream` can be obtained either actively by connecting to a SCTP endpoint with the
/// `connect` constructor, or passively from a `SctpListener` which accepts new connections
pub struct SctpStream(SctpSocket);

impl SctpStream {
	
	/// Create a new stream by connecting it to a remote endpoint
	pub fn connect<A: ToSocketAddrs>(address: A) -> Result<SctpStream> {
		let raw_addr = try!(SocketAddr::from_addr(&address));
		let sock = try!(SctpSocket::new(raw_addr.family(), libc::SOCK_STREAM));
		try!(sock.connect(raw_addr));
		return Ok(SctpStream(sock));
	}
	
	/// Create a new stream by connecting it to a remote endpoint having multiple addresses
	pub fn connectx<A: ToSocketAddrs>(addresses: &[A]) -> Result<SctpStream> {
		if addresses.len() == 0 { return Err(Error::new(ErrorKind::InvalidInput, "No addresses given")); }
		let mut vec = Vec::with_capacity(addresses.len());
		let mut family = libc::AF_INET;
		for address in addresses {
			let a = try!(SocketAddr::from_addr(address));
			if a.family() == libc::AF_INET6 { family = libc::AF_INET6; }
			vec.push(a);
		}

		let sock = try!(SctpSocket::new(family, libc::SOCK_STREAM));
		try!(sock.connectx(&vec));
		return Ok(SctpStream(sock));
	}
	
	/// Send bytes on the specified SCTP stream. On success, returns the
	/// quantity of bytes read
	pub fn sendmsg(&self, msg: &[u8], stream: u16) -> Result<usize> {
		return self.0.sendmsg::<SocketAddr>(msg, None, stream, 0);
	}
	
	/// Read bytes. On success, return a tulpe with the quantity of
	/// bytes received and the stream they were recived on
	pub fn recvmsg(&self, msg: &mut [u8]) -> Result<(usize, u16)> {
		let (size, stream, _) = try!(self.0.recvmsg(msg));
		return Ok((size, stream));
	}
	
	/// Return the list of local socket addresses for this stream
	pub fn local_addrs(&self) -> Result<Vec<SocketAddr>> {
		return self.0.local_addrs(0);
	}
	
	/// Return the list of socket addresses for the peer this stream is connected to
	pub fn peer_addrs(&self) -> Result<Vec<SocketAddr>> {
		return self.0.peer_addrs(0);
	}
	
	/// Shuts down the read, write, or both halves of this connection
	pub fn shutdown(&self, how: Shutdown) -> Result<()> {
		return self.0.shutdown(how);
	}
	
	/// Set or unset SCTP_NODELAY option
	pub fn set_nodelay(&self, nodelay: bool) -> Result<()> {
		let val: libc::c_int = if nodelay { 1 } else { 0 };
		return self.0.setsockopt(SOL_SCTP, sctp_sys::SCTP_NODELAY, &val);
	}
	
	/// Verify if SCTP_NODELAY option is activated for this socket
	pub fn has_nodelay(&self) -> Result<bool> {
		let val: libc::c_int = try!(self.0.sctp_opt_info(sctp_sys::SCTP_NODELAY, 0));
		return Ok(val == 1);
	}
	
	/// Set the socket buffer size for the buffer specifid by `buf`.
	/// Linux system will double the provided size
	pub fn set_buffer_size(&self, buf: SoBuffer, size: usize) -> Result<()> {
		return self.0.setsockopt(libc::SOL_SOCKET, buf.optname(), &(size as libc::c_int));
	}
	
	/// Get the socket buffer size for the buffer specifid by `buf`
	pub fn get_buffer_size(&self, buf: SoBuffer) -> Result<(usize)> {
		let val: u32 = try!(self.0.getsockopt(libc::SOL_SOCKET, buf.optname()));
		return Ok(val as usize);
	}
	
	/// Try to clone the SctpStream. On success, returns a new stream
	/// wrapping a new socket handler
	pub fn try_clone(&self) -> Result<SctpStream> {
		return Ok(SctpStream(try!(self.0.try_clone())));
	}
}

impl Read for SctpStream {
	fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
		return self.0.recv(buf);
	}
}

impl Write for SctpStream {
	fn write(&mut self, buf: &[u8]) -> Result<usize> {
		return self.0.send(buf);
	}
	
	fn flush(&mut self) -> Result<()> {
		return Ok(());
	}
}

#[cfg(target_os="windows")]
impl AsRawHandle for SctpStream {
	fn as_raw_handle(&self) -> RawHandle {
		return return self.0.as_raw_handle();	
	}
}

#[cfg(target_os="windows")]
impl FromRawHandle for SctpStream {
	unsafe fn from_raw_handle(hdl: RawHandle) -> SctpStream {
		return SctpStream(SctpSocket::from_raw_handle(hdl));
	}
}

#[cfg(target_os="linux")]
impl AsRawFd for SctpStream {
	fn as_raw_fd(&self) -> RawFd {
		return self.0.as_raw_fd();	
	}
}

#[cfg(target_os="linux")]
impl FromRawFd for SctpStream {
	unsafe fn from_raw_fd(fd: RawFd) -> SctpStream {
		return SctpStream(SctpSocket::from_raw_fd(fd));
	}
}


/// One-to-many SCTP endpoint.
pub struct SctpEndpoint(SctpSocket);

impl SctpEndpoint {
	
	/// Create a one-to-many SCTP endpoint bound to a single address
	pub fn bind<A: ToSocketAddrs>(address: A) -> Result<SctpEndpoint> {
		let raw_addr = try!(SocketAddr::from_addr(&address));
		let sock = try!(SctpSocket::new(raw_addr.family(), libc::SOCK_STREAM));
		try!(sock.bind(raw_addr));
		return Ok(SctpEndpoint(sock));
	}
	
	/// Create a one-to-many SCTP endpoint bound to a multiple addresses. Requires at least one address
	pub fn bindx<A: ToSocketAddrs>(addresses: &[A]) -> Result<SctpEndpoint> {
		if addresses.len() == 0 { return Err(Error::new(ErrorKind::InvalidInput, "No addresses given")); }
		let mut vec = Vec::with_capacity(addresses.len());
		let mut family = libc::AF_INET;
		for address in addresses {
			let a = try!(SocketAddr::from_addr(address));
			if a.family() == libc::AF_INET6 { family = libc::AF_INET6; }
			vec.push(a);
		}

		let sock = try!(SctpSocket::new(family, SOCK_SEQPACKET));
		try!(sock.bindx(&vec, BindOp::AddAddr));
		try!(sock.listen(-1));
		return Ok(SctpEndpoint(sock));
	}
	
	/// Wait for data to be received. On success, returns a triplet containing
	/// the quantity of bytes received, the sctp stream id on which data were received, and
	/// the socket address used by the peer to send the data
	pub fn recv_from(&self, msg: &mut [u8]) -> Result<(usize, u16, SocketAddr)> {
		return self.0.recvmsg(msg);
	}
	
	/// Send data in Sctp style, to the provided address on the stream `stream`.
	/// On success, returns the quantity on bytes sent
	pub fn send_to<A: ToSocketAddrs>(&self, msg: &mut [u8], address: A, stream: u16) -> Result<usize> {
		return self.0.sendmsg(msg, Some(address), stream, 0);
	}
	
	/// Get local socket addresses on which this socket is bound
	pub fn local_addrs(&self) -> Result<Vec<SocketAddr>> {
		return self.0.local_addrs(0);
	}
	
	/// Try to clone this socket
	pub fn try_clone(&self) -> Result<SctpEndpoint> {
		return Ok(SctpEndpoint(try!(self.0.try_clone())));
	}
}

#[cfg(target_os="windows")]
impl AsRawHandle for SctpEndpoint {
	fn as_raw_handle(&self) -> RawHandle {
		return return self.0.as_raw_handle();	
	}
}

#[cfg(target_os="windows")]
impl FromRawHandle for SctpEndpoint {
	unsafe fn from_raw_handle(hdl: RawHandle) -> SctpEndpoint {
		return SctpEndpoint(SctpSocket::from_raw_handle(hdl));
	}
}

#[cfg(target_os="linux")]
impl AsRawFd for SctpEndpoint {
	fn as_raw_fd(&self) -> RawFd {
		return self.0.as_raw_fd();	
	}
}

#[cfg(target_os="linux")]
impl FromRawFd for SctpEndpoint {
	unsafe fn from_raw_fd(fd: RawFd) -> SctpEndpoint {
		return SctpEndpoint(SctpSocket::from_raw_fd(fd));
	}
}

/// Iterator over incoming connections on `SctpListener`
pub struct Incoming<'a>(&'a SctpListener);

impl <'a> std::iter::Iterator for Incoming<'a> {
	type Item = Result<SctpStream>;
	
	fn next(&mut self) -> Option<Result<SctpStream>> {
		return match self.0.accept() {
			Ok((stream, _)) => Some(Ok(stream)),
			Err(e) => Some(Err(e))
		};
	}
}


/// SCTP listener which behaves like a `TcpListener`.
/// A SCTP listener is used to wait for and accept one-to-one SCTP connections.
/// An accepted connection is represented by `SctpStream`.
pub struct SctpListener(SctpSocket);

impl SctpListener {
	
	/// Create a listener bound to a single address
	pub fn bind<A: ToSocketAddrs>(address: A) -> Result<SctpListener> {
		let raw_addr = try!(SocketAddr::from_addr(&address));
		let sock = try!(SctpSocket::new(raw_addr.family(), libc::SOCK_STREAM));
		try!(sock.bind(raw_addr));
		return Ok(SctpListener(sock));
	}
	
	/// Create a listener bound to multiple addresses. Requires at least one address
	pub fn bindx<A: ToSocketAddrs>(addresses: &[A]) -> Result<SctpListener> {
		if addresses.len() == 0 { return Err(Error::new(ErrorKind::InvalidInput, "No addresses given")); }
		let mut vec = Vec::with_capacity(addresses.len());
		let mut family = libc::AF_INET;
		for address in addresses {
			let a = try!(SocketAddr::from_addr(address));
			if a.family() == libc::AF_INET6 { family = libc::AF_INET6; }
			vec.push(a);
		}

		let sock = try!(SctpSocket::new(family, libc::SOCK_STREAM));
		try!(sock.bindx(&vec, BindOp::AddAddr));
		try!(sock.listen(-1));
		return Ok(SctpListener(sock));
	}
	
	/// Accept a new connection
	pub fn accept(&self) -> Result<(SctpStream, SocketAddr)> {
		let (sock, addr) = try!(self.0.accept());
		return Ok((SctpStream(sock), addr));
	}
	
	/// Iterate over new connections.
	pub fn incoming(&self) -> Incoming {
		return Incoming(self);
	}
	
	/// Get the listener ocal addresses
	pub fn local_addrs(&self) -> Result<Vec<SocketAddr>> {
		return self.0.local_addrs(0);
	}
	
	/// Try to clone this listener
	pub fn try_clone(&self) -> Result<SctpListener> {
		return Ok(SctpListener(try!(self.0.try_clone())));
	}
}

#[cfg(target_os="windows")]
impl AsRawHandle for SctpListener {
	fn as_raw_handle(&self) -> RawHandle {
		return return self.0.as_raw_handle();	
	}
}

#[cfg(target_os="windows")]
impl FromRawHandle for SctpListener {
	unsafe fn from_raw_handle(hdl: RawHandle) -> SctpListener {
		return SctpListener(SctpSocket::from_raw_handle(hdl));
	}
}

#[cfg(target_os="linux")]
impl AsRawFd for SctpListener {
	fn as_raw_fd(&self) -> RawFd {
		return self.0.as_raw_fd();	
	}
}

#[cfg(target_os="linux")]
impl FromRawFd for SctpListener {
	unsafe fn from_raw_fd(fd: RawFd) -> SctpListener {
		return SctpListener(SctpSocket::from_raw_fd(fd));
	}
}
