use std::cmp::{Ord, Ordering};

#[derive(Eq, Clone)]
pub enum NetworkInfo {
    Ethernet,
    Wifi(String, u32, bool),
}

impl Ord for NetworkInfo {
    fn cmp(&self, other: &NetworkInfo) -> Ordering {
        if let NetworkInfo::Wifi(ref essid1, ref quality1, _) = *self {
            if let NetworkInfo::Wifi(ref essid2, ref quality2, _) = other {
                let t = quality2.cmp(quality1);
                if t == Ordering::Equal {
                    essid1.cmp(essid2)
                } else {
                    t
                }
            } else {
                Ordering::Greater
            }
        } else {
            Ordering::Less
        }
    }
}

impl PartialEq for NetworkInfo {
    fn eq(&self, other: &NetworkInfo) -> bool {
        if let NetworkInfo::Wifi(ref essid1, _, _) = *self {
            if let NetworkInfo::Wifi(ref essid2, _, _) = other {
                return essid1 == essid2;
            }
        }
        false
    }
}

impl PartialOrd for NetworkInfo {
    fn partial_cmp(&self, other: &NetworkInfo) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
