use super::connection::{ConnectionInfo, KnownNetwork, NetworkInfo, NetworkList};

use rustbus::{
    signature,
    wire::{marshal::MarshalContext, util::insert_u32},
    Error, Marshal, Signature,
};

impl Signature for ConnectionInfo {
    fn signature() -> signature::Type {
        signature::Type::Container(signature::Container::Struct(vec![
            u32::signature(),
            String::signature(),
            bool::signature(),
            u32::signature(),
            String::signature(),
        ]))
    }

    fn alignment() -> usize {
        8
    }
}

impl Marshal for ConnectionInfo {
    fn marshal(&self, ctx: &mut MarshalContext) -> Result<(), Error> {
        ctx.align_to(Self::alignment());
        match self {
            ConnectionInfo::NotConnected => {
                0.marshal(ctx)?;
                "".marshal(ctx)?;
                false.marshal(ctx)?;
                0.marshal(ctx)?;
                "".marshal(ctx)?;
            }
            ConnectionInfo::Ethernet(ip) => {
                1.marshal(ctx)?;
                "Ethernet connection".marshal(ctx)?;
                false.marshal(ctx)?;
                100.marshal(ctx)?;
                ip.marshal(ctx)?;
            }
            ConnectionInfo::Wifi(essid, quality, enc, ip) => {
                2.marshal(ctx)?;
                essid.marshal(ctx)?;
                enc.marshal(ctx)?;
                quality.marshal(ctx)?;
                ip.marshal(ctx)?;
            }
            ConnectionInfo::ConnectingEth => {
                3.marshal(ctx)?;
                "Ethernet connection".marshal(ctx)?;
                false.marshal(ctx)?;
                100.marshal(ctx)?;
                "".marshal(ctx)?;
            }
            ConnectionInfo::ConnectingWifi(essid) => {
                4.marshal(ctx)?;
                essid.marshal(ctx)?;
                false.marshal(ctx)?;
                0.marshal(ctx)?;
                "".marshal(ctx)?;
            }
        }
        Ok(())
    }
}

impl Signature for KnownNetwork {
    fn signature() -> signature::Type {
        signature::Type::Container(signature::Container::Struct(vec![
            String::signature(),
            i32::signature(),
            bool::signature(),
            bool::signature(),
            bool::signature(),
        ]))
    }

    fn alignment() -> usize {
        8
    }
}

impl Marshal for KnownNetwork {
    fn marshal(&self, ctx: &mut MarshalContext) -> Result<(), Error> {
        ctx.align_to(Self::alignment());
        self.password
            .clone()
            .unwrap_or("".to_owned())
            .marshal(ctx)?;
        self.threshold.unwrap_or(-65).marshal(ctx)?;
        self.auto.marshal(ctx)?;
        self.password.is_some().marshal(ctx)?;
        self.threshold.is_some().marshal(ctx)?;
        Ok(())
    }
}

impl Signature for NetworkInfo {
    fn signature() -> signature::Type {
        signature::Type::Container(signature::Container::Struct(vec![
            u32::signature(),
            String::signature(),
            bool::signature(),
            u32::signature(),
        ]))
    }

    fn alignment() -> usize {
        8
    }
}

impl Marshal for NetworkInfo {
    fn marshal(&self, ctx: &mut MarshalContext) -> Result<(), Error> {
        ctx.align_to(Self::alignment());
        match self {
            NetworkInfo::Ethernet => {
                1.marshal(ctx)?;
                "Ethernet connection".marshal(ctx)?;
                false.marshal(ctx)?;
                100.marshal(ctx)?;
            }
            NetworkInfo::Wifi(essid, quality, enc) => {
                2.marshal(ctx)?;
                essid.marshal(ctx)?;
                enc.marshal(ctx)?;
                quality.marshal(ctx)?;
            }
        }
        Ok(())
    }
}

impl Signature for NetworkList {
    fn signature() -> signature::Type {
        signature::Type::Container(signature::Container::Array(Box::new(
            NetworkInfo::signature(),
        )))
    }

    fn alignment() -> usize {
        NetworkInfo::alignment()
    }
}

impl Marshal for NetworkList {
    fn marshal(&self, ctx: &mut MarshalContext) -> Result<(), Error> {
        ctx.align_to(4);
        let len_pos = ctx.buf.len();
        ctx.buf.push(0);
        ctx.buf.push(0);
        ctx.buf.push(0);
        ctx.buf.push(0);
        ctx.align_to(Self::alignment());

        let content_pos = ctx.buf.len();
        for network in self.iter() {
            network.marshal(ctx)?;
        }

        let len = ctx.buf.len() - content_pos;
        insert_u32(
            ctx.byteorder,
            len as u32,
            &mut ctx.buf[len_pos..len_pos + 4],
        );
        Ok(())
    }
}
