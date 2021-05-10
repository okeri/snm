use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};
use std::{
    convert::TryFrom,
    os::unix::{
        io::{AsRawFd, FromRawFd, RawFd},
        net::UnixStream,
    },
};

use rustbus::{
    auth,
    connection::Error,
    get_session_bus_path, get_system_bus_path,
    message_builder::MarshalledMessage,
    params::message::Message,
    standard_messages,
    wire::{marshal, unmarshal, util},
    ByteOrder, Marshal, MessageBuilder, MessageType,
};

use nix::sys::socket::{
    self, connect, recvmsg, sendmsg, socket, ControlMessage, MsgFlags, SockAddr, UnixAddr,
};
use nix::{cmsg_space, sys::uio::IoVec};

#[allow(dead_code)]
pub enum Bus {
    Session,
    System,
}

#[derive(Debug)]
struct BasicConnection {
    stream: UnixStream,
    msg_buf_in: Vec<u8>,
    msg_buf_out: Vec<u8>,
    serial_counter: Arc<AtomicU32>,
}

impl Clone for BasicConnection {
    fn clone(&self) -> Self {
        let stream = self.stream.try_clone().expect("cannot share UnixStream");
        BasicConnection {
            stream,
            msg_buf_in: Vec::new(),
            msg_buf_out: Vec::new(),
            serial_counter: self.serial_counter.clone(),
        }
    }
}

type Result<T> = std::result::Result<T, Error>;

/// Actual clone of client_conn::Conn
impl BasicConnection {
    pub fn connect_to_bus(addr: UnixAddr) -> Result<BasicConnection> {
        let sock = socket(
            socket::AddressFamily::Unix,
            socket::SockType::Stream,
            socket::SockFlag::empty(),
            None,
        )?;

        let sock_addr = SockAddr::Unix(addr);
        connect(sock, &sock_addr)?;
        let mut stream = unsafe { UnixStream::from_raw_fd(sock) };

        match auth::do_auth(&mut stream)? {
            auth::AuthResult::Ok => {}
            auth::AuthResult::Rejected => return Err(Error::AuthFailed),
        }
        match auth::negotiate_unix_fds(&mut stream)? {
            auth::AuthResult::Ok => {}
            auth::AuthResult::Rejected => return Err(Error::UnixFdNegotiationFailed),
        }

        auth::send_begin(&mut stream)?;
        Ok(BasicConnection {
            stream,
            msg_buf_in: Vec::new(),
            msg_buf_out: Vec::new(),
            serial_counter: Arc::new(AtomicU32::new(1)),
        })
    }

    fn refill_buffer(&mut self, max_buffer_size: usize) -> Result<()> {
        let bytes_to_read = max_buffer_size - self.msg_buf_in.len();

        const BUFSIZE: usize = 512;
        let mut tmpbuf = [0u8; BUFSIZE];
        let iovec = IoVec::from_mut_slice(&mut tmpbuf[..usize::min(bytes_to_read, BUFSIZE)]);

        let mut cmsgspace = cmsg_space!([RawFd; 10]);
        let flags = MsgFlags::empty();

        let msg = recvmsg(
            self.stream.as_raw_fd(),
            &[iovec],
            Some(&mut cmsgspace),
            flags,
        )
        .map_err(|e| match e.as_errno() {
            Some(nix::errno::Errno::EAGAIN) => Error::TimedOut,
            _ => Error::NixError(e),
        });

        self.stream.set_nonblocking(false)?;
        let msg = msg?;

        self.msg_buf_in
            .extend(&mut tmpbuf[..msg.bytes].iter().copied());
        Ok(())
    }

    pub fn bytes_needed_for_current_message(&self) -> Result<usize> {
        if self.msg_buf_in.len() < 16 {
            return Ok(16);
        }
        let (_, header) = unmarshal::unmarshal_header(&self.msg_buf_in, 0)?;
        let (_, header_fields_len) =
            util::parse_u32(&self.msg_buf_in[unmarshal::HEADER_LEN..], header.byteorder)?;
        let complete_header_size = unmarshal::HEADER_LEN + header_fields_len as usize + 4; // +4 because the length of the header fields does not count

        let padding_between_header_and_body = 8 - ((complete_header_size) % 8);
        let padding_between_header_and_body = if padding_between_header_and_body == 8 {
            0
        } else {
            padding_between_header_and_body
        };

        let bytes_needed = complete_header_size as usize
            + padding_between_header_and_body
            + header.body_len as usize;
        Ok(bytes_needed)
    }

    // Checks if the internal buffer currently holds a complete message
    pub fn buffer_contains_whole_message(&self) -> Result<bool> {
        if self.msg_buf_in.len() < 16 {
            return Ok(false);
        }
        let bytes_needed = self.bytes_needed_for_current_message();
        match bytes_needed {
            Err(e) => {
                if let Error::UnmarshalError(unmarshal::Error::NotEnoughBytes) = e {
                    Ok(false)
                } else {
                    Err(e)
                }
            }
            Ok(bytes_needed) => Ok(self.msg_buf_in.len() >= bytes_needed),
        }
    }

    pub fn read_whole_message(&mut self) -> Result<()> {
        while !self.buffer_contains_whole_message()? {
            self.refill_buffer(self.bytes_needed_for_current_message()?)?;
        }
        Ok(())
    }

    pub fn get_next_message(&mut self) -> Result<MarshalledMessage> {
        self.read_whole_message()?;
        let (hdrbytes, header) = unmarshal::unmarshal_header(&self.msg_buf_in, 0)?;
        let (dynhdrbytes, dynheader) =
            unmarshal::unmarshal_dynamic_header(&header, &self.msg_buf_in, hdrbytes)?;

        let (bytes_used, msg) = unmarshal::unmarshal_next_message(
            &header,
            dynheader,
            &self.msg_buf_in,
            hdrbytes + dynhdrbytes,
        )?;

        if self.msg_buf_in.len() != bytes_used + hdrbytes + dynhdrbytes {
            return Err(Error::UnmarshalError(unmarshal::Error::NotAllBytesUsed));
        }
        self.msg_buf_in.clear();
        Ok(msg)
    }

    pub fn send_message(&mut self, mut msg: MarshalledMessage) -> Result<u32> {
        self.msg_buf_out.clear();

        if msg.dynheader.serial.is_none() {
            msg.dynheader.serial = Some(self.serial_counter.fetch_add(1, Ordering::SeqCst));
        }

        marshal::marshal(&msg, ByteOrder::LittleEndian, &[], &mut self.msg_buf_out)?;

        let iov = [IoVec::from_slice(&self.msg_buf_out)];
        let flags = MsgFlags::empty();

        let raw_fds = Vec::new();
        let l = sendmsg(
            self.stream.as_raw_fd(),
            &iov,
            &[ControlMessage::ScmRights(&raw_fds)],
            flags,
            None,
        );

        self.stream.set_nonblocking(false)?;

        let l = l?;

        assert_eq!(l, self.msg_buf_out.len());
        msg.dynheader.serial.ok_or(Error::TimedOut)
    }
}

#[derive(Clone)]
pub struct Emitter {
    connection: BasicConnection,
    iface: String,
    object: String,
}

impl Emitter {
    pub fn emit<P: Marshal>(&mut self, member: &str, param: P) -> Result<u32> {
        let mut sig = MessageBuilder::new()
            .signal(self.iface.clone(), member.into(), self.object.clone())
            .build();
        sig.body.push_param(param)?;
        self.connection.send_message(sig)
    }
}

pub struct DBusLoop {
    connection: BasicConnection,
    iface: String,
}

impl DBusLoop {
    pub fn connect_to_bus(bus: Bus, iface_name: &str) -> Result<DBusLoop> {
        let path = match bus {
            Bus::Session => get_session_bus_path()?,
            Bus::System => get_system_bus_path()?,
        };

        let mut connection = BasicConnection::connect_to_bus(path)?;
        Self::send_message_blocking(&mut connection, standard_messages::hello())?;
        Self::send_message_blocking(
            &mut connection,
            standard_messages::request_name(
                iface_name.into(),
                standard_messages::DBUS_NAME_FLAG_REPLACE_EXISTING,
            ),
        )?;
        Ok(DBusLoop {
            connection,
            iface: iface_name.to_owned(),
        })
    }

    pub fn new_emitter(&self, object: &str) -> Emitter {
        Emitter {
            connection: self.connection.clone(),
            iface: self.iface.clone(),
            object: object.to_owned(),
        }
    }

    pub fn run<Handler>(&mut self, mut handler: Handler) -> Result<()>
    where
        Handler: for<'a, 'e> FnMut(Message<'a, 'e>) -> Option<MarshalledMessage>,
    {
        loop {
            let msg = self.connection.get_next_message()?;
            let msg = msg.unmarshall_all()?;
            if let Some(response) = handler(msg) {
                self.connection.send_message(response)?;
            }
        }
    }

    pub fn add_match(&mut self, m: &str) -> Result<()> {
        self.connection
            .send_message(standard_messages::add_match(m.into()))
            .map(|_| ())
    }

    fn send_message_blocking(
        connection: &mut BasicConnection,
        msg: MarshalledMessage,
    ) -> Result<MarshalledMessage> {
        let serial = connection.send_message(msg)?;
        loop {
            let msg = connection.get_next_message()?;
            match msg.typ {
                MessageType::Reply => {
                    if let Some(ser) = msg.dynheader.response_serial {
                        if ser == serial {
                            return Ok(msg);
                        }
                    }
                }
                MessageType::Error => {
                    let umsg = msg.unmarshall_all()?;
                    if let rustbus::params::Param::Base(ref base) = umsg.params[0] {
                        panic!("{}", String::try_from(base).unwrap());
                    }
                    panic!("unknown bus error");
                }
                _ => {}
            }
        }
    }
}
