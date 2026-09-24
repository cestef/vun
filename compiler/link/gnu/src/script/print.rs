//! The one writer of linker scripts.

use std::fmt::{self, Write};

use super::{Inputs, OutputSection, Pattern, Region, Script};

impl fmt::Display for Script {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(entry) = self.entry {
            writeln!(f, "ENTRY({entry})")?;
        }
        f.write_str("MEMORY\n{\n")?;
        for region in &self.memory {
            writeln!(f, "    {region}")?;
        }
        f.write_str("}\nSECTIONS\n{\n")?;
        for section in &self.sections {
            writeln!(f, "    {section}")?;
        }
        f.write_str("}\n")
    }
}

impl fmt::Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} : ORIGIN = {:#x}, LENGTH = {:#x}", self.name, self.origin, self.length)
    }
}

impl fmt::Display for OutputSection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} : {{ {} }} > {}", self.name, self.inputs, self.region)?;
        match self.load {
            Some(load) => write!(f, " AT > {load}"),
            None => Ok(()),
        }
    }
}

impl fmt::Display for Inputs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.keep {
            f.write_str("KEEP(")?;
        }
        f.write_str("*(")?;
        for (i, pattern) in self.patterns.iter().enumerate() {
            if i > 0 {
                f.write_char(' ')?;
            }
            match pattern {
                Pattern::Exact(name) => write!(f, "{name}")?,
                Pattern::Under(name) => write!(f, "{name}.*")?,
            }
        }
        f.write_char(')')?;
        if self.keep {
            f.write_char(')')?;
        }
        Ok(())
    }
}
