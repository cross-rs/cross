use std::fmt;
use std::str::FromStr;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Runner {
    Native,
    Node,
    None,
    QemuSystem,
    QemuUser,
    Wine,
}

impl Runner {
    pub fn is_none(&self) -> bool {
        self == &Runner::None
    }
}

impl FromStr for Runner {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "native" => Ok(Runner::Native),
            "node" => Ok(Runner::Node),
            "none" => Ok(Runner::None),
            "qemu-system" => Ok(Runner::QemuSystem),
            "qemu-user" => Ok(Runner::QemuUser),
            "wine" => Ok(Runner::Wine),
            _ => Err("invalid runner")
        }
    }
}

impl fmt::Display for Runner {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match *self {
            Runner::Native => "native",
            Runner::Node => "node",
            Runner::None => "none",
            Runner::QemuSystem => "qemu-system",
            Runner::QemuUser => "qemu-user",
            Runner::Wine => "wine"
        };
        write!(f, "{}", s)
    }
}
