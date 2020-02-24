use std::io::Read;
use std::os::unix::{
    io::{AsRawFd, RawFd},
    net::UnixStream,
};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};

use rustbus::{
    auth,
    client_conn::Error,
    get_session_bus_path, get_system_bus_path, message, standard_messages,
    wire::{marshal, unmarshal, util},
    MessageBuilder, MessageType,
};

use nix::sys::socket::{recvmsg, sendmsg, ControlMessage, ControlMessageOwned, MsgFlags};
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

impl BasicConnection {
    pub fn connect_to_bus(path: PathBuf) -> Result<BasicConnection> {
        let mut stream = UnixStream::connect(&path)?;
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

    fn refill_buffer(&mut self, max_buffer_size: usize) -> Result<Vec<ControlMessageOwned>> {
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
        )?;
        let cmsgs = msg.cmsgs().collect();

        self.msg_buf_in
            .extend(&mut tmpbuf[..msg.bytes].iter().copied());
        Ok(cmsgs)
    }

    /// Blocks until a message has been read from the conn
    pub fn get_next_message(&mut self) -> Result<message::Message> {
        // This whole dance around reading exact amounts of bytes is necessary to read messages exactly at their bounds.
        // I think thats necessary so we can later add support for unixfd sending
        let mut cmsgs = Vec::new();

        let header = loop {
            match unmarshal::unmarshal_header(&self.msg_buf_in, 0) {
                Ok((_, header)) => break header,
                Err(unmarshal::Error::NotEnoughBytes) => {}
                Err(e) => return Err(Error::from(e)),
            }
            let new_cmsgs = self.refill_buffer(unmarshal::HEADER_LEN)?;
            cmsgs.extend(new_cmsgs);
        };

        let mut header_fields_len = [0u8; 4];
        self.stream.read_exact(&mut header_fields_len[..])?;
        let (_, header_fields_len) =
            util::parse_u32(&header_fields_len.to_vec(), header.byteorder)?;
        util::write_u32(header_fields_len, header.byteorder, &mut self.msg_buf_in);

        let complete_header_size = unmarshal::HEADER_LEN + header_fields_len as usize + 4; // +4 because the length of the header fields does not count

        let padding_between_header_and_body = 8 - ((complete_header_size) % 8);
        let padding_between_header_and_body = if padding_between_header_and_body == 8 {
            0
        } else {
            padding_between_header_and_body
        };

        let bytes_needed = unmarshal::HEADER_LEN
            + (header.body_len + header_fields_len + 4) as usize
            + padding_between_header_and_body; // +4 because the length of the header fields does not count
        loop {
            let new_cmsgs = self.refill_buffer(bytes_needed)?;
            cmsgs.extend(new_cmsgs);
            if self.msg_buf_in.len() == bytes_needed {
                break;
            }
        }
        let (bytes_used, mut msg) = unmarshal::unmarshal_next_message(
            &header,
            &mut self.msg_buf_in,
            unmarshal::HEADER_LEN,
        )?;
        if bytes_needed != bytes_used + unmarshal::HEADER_LEN {
            return Err(Error::UnmarshalError(unmarshal::Error::NotAllBytesUsed));
        }
        self.msg_buf_in.clear();

        for cmsg in cmsgs {
            match cmsg {
                ControlMessageOwned::ScmRights(fds) => {
                    msg.raw_fds.extend(fds);
                }
                _ => {
                    // TODO what to do?
                    println!("Cmsg other than ScmRights: {:?}", cmsg);
                }
            }
        }
        Ok(msg)
    }

    pub fn send_message(&mut self, mut msg: message::Message) -> Result<message::Message> {
        self.msg_buf_out.clear();
        if msg.serial.is_none() {
            msg.serial = Some(self.serial_counter.fetch_add(1, Ordering::SeqCst));
        }
        marshal::marshal(
            &msg,
            message::ByteOrder::LittleEndian,
            &[],
            &mut self.msg_buf_out,
        )?;
        let iov = [IoVec::from_slice(&self.msg_buf_out)];
        let flags = MsgFlags::empty();

        let l = sendmsg(
            self.stream.as_raw_fd(),
            &iov,
            &[ControlMessage::ScmRights(&msg.raw_fds)],
            flags,
            None,
        )?;
        assert_eq!(l, self.msg_buf_out.len());
        Ok(msg)
    }
}

#[derive(Clone)]
pub struct Emitter {
    connection: BasicConnection,
    iface: String,
    object: String,
}

impl Emitter {
    pub fn emit(&mut self, member: &str, args: Vec<message::Param>) -> Result<message::Message> {
        let sig = MessageBuilder::new()
            .signal(self.iface.clone(), member.into(), self.object.clone())
            .with_params(args)
            .build();
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
            standard_messages::request_name(iface_name.into(), 0),
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
        Handler: FnMut(message::Message) -> Option<message::Message>,
    {
        loop {
            let msg = self.connection.get_next_message()?;
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
        msg: message::Message,
    ) -> Result<message::Message> {
        let sent = connection.send_message(msg)?;
        let serial = sent.serial.unwrap();
        loop {
            let msg = connection.get_next_message()?;
            if let MessageType::Reply = msg.typ {
                if let Some(ser) = msg.response_serial {
                    if ser == serial {
                        return Ok(msg);
                    }
                }
            }
        }
    }
}
