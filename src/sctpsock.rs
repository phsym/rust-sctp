use libc;
use sctp_sys;
use std;

use std::io::{Error, ErrorKind, Read, Result, Write};
use std::mem::{size_of, MaybeUninit};
use std::net::{
    Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr, SocketAddrV4, SocketAddrV6, ToSocketAddrs,
};

// import macros from lib
#[cfg(target_os = "linux")]
use crate::{sctp_syscall, syscall};

// import sockaddr helpers from lib
#[cfg(target_os = "linux")]
use crate::mio_unix::{socket_addr, to_socket_addr};

#[cfg(target_os = "linux")]
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};

#[cfg(target_os = "windows")]
use std::os::windows::io::{AsRawHandle, FromRawHandle, RawHandle};

#[cfg(target_os = "windows")]
mod win {
    use libc;
    use std::io::{Error, Result};
    use winapi;

    pub use winapi::{
        sockaddr_in6, socklen_t, AF_INET, AF_INET6, SOCKADDR as sockaddr,
        SOCKADDR_IN as sockaddr_in, SOCKET,
    };
    pub use ws2_32::{closesocket, socket};

    pub type RWlen = i32;

    pub const SHUT_RD: libc::c_int = 0;
    pub const SHUT_WR: libc::c_int = 1;
    pub const SHUT_RDWR: libc::c_int = 2;

    pub fn check_socket(sock: SOCKET) -> Result<SOCKET> {
        if sock == winapi::INVALID_SOCKET {
            return Err(Error::last_os_error());
        }
        return Ok(sock);
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use libc;
    use std::io::{Error, Result};

    pub use libc::{
        sockaddr, sockaddr_in, sockaddr_in6, socket, socklen_t, AF_INET, AF_INET6, EINPROGRESS,
        SHUT_RD, SHUT_RDWR, SHUT_WR,
    };

    pub type SOCKET = libc::c_int;
    pub type RWlen = libc::size_t;

    pub unsafe fn closesocket(sock: SOCKET) {
        libc::close(sock);
    }

    pub fn check_socket(sock: SOCKET) -> Result<SOCKET> {
        if sock < 0 {
            return Err(Error::last_os_error());
        }
        return Ok(sock);
    }
}

#[cfg(target_os = "linux")]
use self::linux::*;
#[cfg(target_os = "windows")]
use self::win::*;

/// SCTP bind operation
#[allow(dead_code)]
pub enum BindOp {
    /// Add bind addresses
    AddAddr,
    /// Remove bind addresses
    RemAddr,
}

impl BindOp {
    fn flag(&self) -> libc::c_int {
        return match *self {
            BindOp::AddAddr => sctp_sys::SCTP_BINDX_ADD_ADDR,
            BindOp::RemAddr => sctp_sys::SCTP_BINDX_REM_ADDR,
        };
    }
}

enum SctpAddrType {
    Local,
    Peer,
}

impl SctpAddrType {
    unsafe fn get(
        &self,
        sock: SOCKET,
        id: sctp_sys::sctp_assoc_t,
        ptr: *mut *mut sockaddr,
    ) -> libc::c_int {
        return match *self {
            SctpAddrType::Local => sctp_sys::sctp_getladdrs(sock, id, ptr),
            SctpAddrType::Peer => sctp_sys::sctp_getpaddrs(sock, id, ptr),
        };
    }

    unsafe fn free(&self, ptr: *mut sockaddr) {
        return match *self {
            SctpAddrType::Local => sctp_sys::sctp_freeladdrs(ptr),
            SctpAddrType::Peer => sctp_sys::sctp_freepaddrs(ptr),
        };
    }
}

/// Manage low level socket address structure
pub trait RawSocketAddr: Sized {
    /// Get the address family for this socket address
    fn family(&self) -> i32;

    /// Create from a raw socket address
    unsafe fn from_raw_ptr(addr: *const sockaddr, len: socklen_t) -> Result<Self>;

    /// Create from a ToSocketAddrs
    fn from_addr<A: ToSocketAddrs>(address: A) -> Result<Self>;
}

impl RawSocketAddr for SocketAddr {
    fn family(&self) -> i32 {
        return match *self {
            SocketAddr::V4(..) => AF_INET,
            SocketAddr::V6(..) => AF_INET6,
        };
    }

    unsafe fn from_raw_ptr(addr: *const sockaddr, len: socklen_t) -> Result<SocketAddr> {
        if len < size_of::<sockaddr>() as socklen_t {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Invalid address length",
            ));
        }
        match (*addr).sa_family as libc::c_int {
            AF_INET => {
                let in_addr = std::ptr::read(addr as *const sockaddr_in);
                let ip_addr = Ipv4Addr::from(in_addr.sin_addr.s_addr.to_be());
                let socket_addr_v4 = SocketAddrV4::new(ip_addr, u16::from_be(in_addr.sin_port));
                return Ok(SocketAddr::V4(socket_addr_v4));
            }
            AF_INET6 if len >= size_of::<sockaddr_in6>() as socklen_t => {
                let in6_addr = std::ptr::read(addr as *const sockaddr_in6);
                let ip6_addr = Ipv6Addr::from(in6_addr.sin6_addr.s6_addr);
                let socket_addr_v6 = SocketAddrV6::new(
                    ip6_addr,
                    u16::from_be(in6_addr.sin6_port),
                    in6_addr.sin6_flowinfo,
                    in6_addr.sin6_scope_id,
                );
                return Ok(SocketAddr::V6(socket_addr_v6));
            }
            _ => Err(Error::new(
                ErrorKind::InvalidInput,
                "Invalid socket address",
            )),
        }
    }

    fn from_addr<A: ToSocketAddrs>(address: A) -> Result<SocketAddr> {
        return address
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "Address is not valid"));
    }
}

/// A High level wrapper around SCTP socket, of any kind
pub struct SctpSocket(SOCKET);

impl SctpSocket {
    /// Create a new SCTP socket
    pub fn new(family: libc::c_int, sock_type: libc::c_int) -> Result<SctpSocket> {
        unsafe {
            return Ok(SctpSocket(check_socket(socket(
                family,
                sock_type,
                sctp_sys::IPPROTO_SCTP,
            ))?));
        }
    }

    /// Connect the socket to `address`
    pub fn connect<A: ToSocketAddrs>(&self, address: A) -> Result<()> {
        let addrobj = SocketAddr::from_addr(&address)?;
        let (raw_addr, raw_addr_length) = socket_addr(&addrobj);
        match syscall!(connect(self.0, raw_addr.as_ptr(), raw_addr_length)) {
            Err(err) if err.raw_os_error() != Some(EINPROGRESS) => Err(err),
            _ => Ok(()),
        }
    }

    /// Connect the socket to multiple addresses
    pub fn connectx<A: ToSocketAddrs>(&self, addresses: &[A]) -> Result<sctp_sys::sctp_assoc_t> {
        if addresses.len() == 0 {
            return Err(Error::new(ErrorKind::InvalidInput, "No addresses given"));
        }

        let buf: *mut u8 = unsafe {
            libc::malloc((addresses.len() * size_of::<libc::sockaddr_storage>()) as libc::size_t)
                as *mut u8
        };
        if buf.is_null() {
            return Err(Error::new(ErrorKind::Other, "Out of memory"));
        }
        let mut offset = 0isize;
        for address in addresses {
            let addrobj = SocketAddr::from_addr(&address)?;
            let (raw_addr, raw_addr_length) = socket_addr(&addrobj);
            unsafe {
                std::ptr::copy_nonoverlapping(
                    raw_addr.as_ptr() as *mut u8,
                    buf.offset(offset),
                    raw_addr_length as usize,
                )
            };
            offset += raw_addr_length as isize;
        }

        let mut assoc: sctp_sys::sctp_assoc_t = 0;

        match sctp_syscall!(sctp_connectx(
            self.0,
            buf as *mut sockaddr,
            addresses.len() as i32,
            &mut assoc
        )) {
            Err(err) => {
                unsafe { libc::free(buf as *mut libc::c_void) };
                Err(err)
            }
            Ok(_) => {
                unsafe { libc::free(buf as *mut libc::c_void) };
                Ok(assoc)
            }
        }
    }

    /// Bind the socket to a single address
    pub fn bind<A: ToSocketAddrs>(&self, address: A) -> Result<()> {
        let addrobj = SocketAddr::from_addr(&address)?;
        let (raw_addr, raw_addr_length) = socket_addr(&addrobj);
        syscall!(bind(self.0, raw_addr.as_ptr(), raw_addr_length))?;
        Ok(())
    }

    /// Bind the socket on multiple addresses
    pub fn bindx<A: ToSocketAddrs>(&self, addresses: &[A], op: BindOp) -> Result<()> {
        if addresses.len() == 0 {
            return Err(Error::new(ErrorKind::InvalidInput, "No addresses given"));
        }

        let buf: *mut u8 = unsafe {
            libc::malloc((addresses.len() * size_of::<sockaddr_in6>()) as libc::size_t) as *mut u8
        };
        if buf.is_null() {
            return Err(Error::new(ErrorKind::Other, "Out of memory"));
        }
        let mut offset = 0isize;
        for address in addresses {
            let addrobj = SocketAddr::from_addr(&address)?;
            let (raw_addr, raw_addr_length) = socket_addr(&addrobj);
            unsafe {
                std::ptr::copy_nonoverlapping(
                    raw_addr.as_ptr() as *mut u8,
                    buf.offset(offset),
                    raw_addr_length as usize,
                )
            };
            offset += raw_addr_length as isize;
        }

        match sctp_syscall!(sctp_bindx(
            self.0,
            buf as *mut sockaddr,
            addresses.len() as i32,
            op.flag()
        )) {
            Err(err) => {
                unsafe { libc::free(buf as *mut libc::c_void) };
                Err(err)
            }
            Ok(_) => Ok(()),
        }
    }

    /// Listen
    pub fn listen(&self, backlog: libc::c_int) -> Result<()> {
        syscall!(listen(self.0, backlog))?;
        Ok(())
    }

    /// Accept connection to this socket
    pub fn accept(&self) -> Result<(SctpSocket, SocketAddr)> {
        // prepare buffer to store client address
        // TODO: this will not be compatible with windows environments as we use libc structs
        let mut addr_storage: MaybeUninit<libc::sockaddr_storage> = MaybeUninit::uninit();
        let mut addr_storage_length = size_of::<libc::sockaddr_storage>() as libc::socklen_t;

        let stream = {
            syscall!(accept(
                self.0,
                addr_storage.as_mut_ptr() as *mut _,
                &mut addr_storage_length
            ))
            .map(|socket| SctpSocket(socket))
        }?;

        unsafe { to_socket_addr(addr_storage.as_ptr()) }.map(|addr| (stream, addr))
    }

    fn addrs(&self, id: sctp_sys::sctp_assoc_t, what: SctpAddrType) -> Result<Vec<SocketAddr>> {
        unsafe {
            // Initialize a pointer that will hold the addresses
            let mut addrs: *mut sockaddr = std::ptr::null_mut();
            let len = what.get(self.0, id, &mut addrs);

            if len < 0 {
                return Err(Error::new(ErrorKind::Other, "Cannot retrieve addresses"));
            }
            if len == 0 {
                return Err(Error::new(ErrorKind::AddrNotAvailable, "Socket is unbound"));
            }

            // Prepare a vector to hold the addresses
            let mut vec = Vec::with_capacity(len as usize);
            let mut offset = 0;
            for _ in 0..len {
                let sockaddr_ptr = addrs.offset(offset) as *const sockaddr;
                let family = (*sockaddr_ptr).sa_family as i32;
                let sockaddr_len = match family {
                    AF_INET => size_of::<sockaddr_in>() as socklen_t,
                    AF_INET6 => size_of::<sockaddr_in6>() as socklen_t,
                    _ => {
                        what.free(addrs);
                        return Err(Error::new(
                            ErrorKind::Other,
                            format!("Unsupported address family : {}", family),
                        ));
                    }
                };

                // convert raw pointer to `SocketAddr`
                vec.push(SocketAddr::from_raw_ptr(sockaddr_ptr, sockaddr_len)?);
                offset += sockaddr_len as isize;
            }

            // free allocated addresses
            what.free(addrs);

            return Ok(vec);
        }
    }

    /// List socket's local addresses
    pub fn local_addrs(&self, id: sctp_sys::sctp_assoc_t) -> Result<Vec<SocketAddr>> {
        return self.addrs(id, SctpAddrType::Local);
    }

    /// Get peer addresses for a connected socket or a given association
    pub fn peer_addrs(&self, id: sctp_sys::sctp_assoc_t) -> Result<Vec<SocketAddr>> {
        return self.addrs(id, SctpAddrType::Peer);
    }

    /// Receive data in TCP style. Only works for a connected one to one socket
    pub fn recv(&mut self, buf: &mut [u8]) -> Result<usize> {
        let len = buf.len() as RWlen;

        match syscall!(recv(self.0, buf.as_mut_ptr() as *mut libc::c_void, len, 0)) {
            Err(err) => Err(err),
            Ok(recvlen) => Ok(recvlen as usize),
        }
    }

    /// Send data in TCP style. Only wmmatorks for a connected one to one socket
    pub fn send(&mut self, buf: &[u8]) -> Result<usize> {
        let len = buf.len() as RWlen;

        match syscall!(send(self.0, buf.as_ptr() as *const libc::c_void, len, 0)) {
            Err(err) => Err(err),
            Ok(recvlen) => Ok(recvlen as usize),
        }
    }

    /// Wait for data to be received. On success, returns a triplet containing
    /// the quantity of bytes received, the sctp stream id on which data were received, and
    /// the socket address used by the peer to send the data
    pub fn recvmsg(&self, msg: &mut [u8]) -> Result<(usize, u16, SocketAddr)> {
        let len = msg.len() as libc::size_t;

        let mut flags: libc::c_int = 0;
        let mut info: sctp_sys::sctp_sndrcvinfo = unsafe { std::mem::zeroed() };

        // prepare buffer to store client address
        // TODO: this will not be compatible with windows environments as we use libc structs
        let mut addr_storage: MaybeUninit<libc::sockaddr_storage> = MaybeUninit::uninit();
        let mut addr_storage_length = size_of::<libc::sockaddr_storage>() as libc::socklen_t;

        let recvlen = sctp_syscall!(sctp_recvmsg(
            self.0,
            msg.as_mut_ptr() as *mut _,
            len,
            addr_storage.as_mut_ptr() as *mut _,
            &mut addr_storage_length,
            &mut info,
            &mut flags
        ))?;

        unsafe { to_socket_addr(addr_storage.as_ptr()) }
            .map(|addr| (recvlen as usize, info.sinfo_stream, addr))
    }

    /// Send data in Sctp style, to the provided address (may be `None` if the socket is connected), on the stream `stream`, with the TTL `ttl`.
    /// On success, returns the quantity on bytes sent
    pub fn sendmsg<A: ToSocketAddrs>(
        &self,
        msg: &[u8],
        address: Option<A>,
        ppid: u32,
        stream: u16,
        ttl: libc::c_ulong,
    ) -> Result<usize> {
        let len = msg.len() as libc::size_t;
        let (raw_addr, addr_len) = match address {
            Some(a) => {
                let addrobj = SocketAddr::from_addr(a)?;
                let (addr_c_struct, addr_c_struct_len) = socket_addr(&addrobj);
                (addr_c_struct.as_ptr() as *mut sockaddr, addr_c_struct_len)
            }
            None => (std::ptr::null_mut(), 0),
        };
        let ppid = ppid.to_be();

        match sctp_syscall!(sctp_sendmsg(
            self.0,
            msg.as_ptr() as *const libc::c_void,
            len,
            raw_addr,
            addr_len,
            ppid as libc::c_ulong,
            0,
            stream,
            ttl,
            0
        )) {
            Err(err) => Err(err),
            Ok(sendlen) => Ok(sendlen as usize),
        }
    }

    /// Shuts down the read, write, or both halves of this connection
    pub fn shutdown(&self, how: Shutdown) -> Result<()> {
        let side = match how {
            Shutdown::Read => SHUT_RD,
            Shutdown::Write => SHUT_WR,
            Shutdown::Both => SHUT_RDWR,
        };
        match syscall!(shutdown(self.0, side)) {
            Err(err) => Err(err),
            Ok(_) => Ok(()),
        }
    }

    /// Set socket option
    pub fn setsockopt<T>(
        &self,
        level: libc::c_int,
        optname: libc::c_int,
        optval: &T,
    ) -> Result<()> {
        let optval_ptr = optval as *const T as *const libc::c_void;
        let optlen = size_of::<T>() as socklen_t;

        match syscall!(setsockopt(self.0, level, optname, optval_ptr, optlen)) {
            Err(err) => Err(err),
            Ok(_) => Ok(()),
        }
    }

    /// Get socket option
    pub fn getsockopt<T>(&self, level: libc::c_int, optname: libc::c_int) -> Result<T> {
        let mut val: T = unsafe { std::mem::zeroed() };

        let mut len = size_of::<T>() as socklen_t;

        match syscall!(getsockopt(
            self.0,
            level,
            optname,
            &mut val as *mut T as *mut libc::c_void,
            &mut len
        )) {
            Err(err) => Err(err),
            Ok(_) => Ok(val),
        }
    }

    /// Get SCTP socket option
    pub fn sctp_opt_info<T>(
        &self,
        optname: libc::c_int,
        assoc: sctp_sys::sctp_assoc_t,
    ) -> Result<T> {
        let mut val: T = unsafe { std::mem::zeroed() };
        let mut len = size_of::<T>() as socklen_t;

        match sctp_syscall!(sctp_opt_info(
            self.0,
            assoc,
            optname,
            &mut val as *mut T as *mut libc::c_void,
            &mut len
        )) {
            Err(err) => Err(err),
            Ok(_) => Ok(val),
        }
    }

    /// Try to clone this socket
    pub fn try_clone(&self) -> Result<SctpSocket> {
        match syscall!(dup(self.0 as i32)) {
            Err(err) => Err(err),
            Ok(new_sock) => Ok(SctpSocket(new_sock as SOCKET)),
        }
    }
}

impl Read for SctpSocket {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        return self.recv(buf);
    }
}

impl Write for SctpSocket {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        return self.send(buf);
    }

    fn flush(&mut self) -> Result<()> {
        return Ok(());
    }
}

#[cfg(target_os = "windows")]
impl AsRawHandle for SctpSocket {
    fn as_raw_handle(&self) -> RawHandle {
        return self.0 as RawHandle;
    }
}

#[cfg(target_os = "windows")]
impl FromRawHandle for SctpSocket {
    unsafe fn from_raw_handle(hdl: RawHandle) -> SctpSocket {
        return SctpSocket(hdl as SOCKET);
    }
}

#[cfg(target_os = "linux")]
impl AsRawFd for SctpSocket {
    fn as_raw_fd(&self) -> RawFd {
        return self.0;
    }
}

#[cfg(target_os = "linux")]
impl FromRawFd for SctpSocket {
    unsafe fn from_raw_fd(fd: RawFd) -> SctpSocket {
        return SctpSocket(fd);
    }
}

impl Drop for SctpSocket {
    fn drop(&mut self) {
        unsafe { closesocket(self.0) };
    }
}
