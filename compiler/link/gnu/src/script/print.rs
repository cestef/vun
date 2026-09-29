//! The one writer of linker scripts.

use std::fmt::{self, Write};

use super::{Inputs, OutputSection, Pattern, Region, Script};

/// Where the location counter stands, as a symbol's value.
const HERE: &str = ".";

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
            if let Some(load) = &section.load {
                writeln!(f, "    {} = LOADADDR({});", load.at, section.name)?;
            }
        }
        f.write_str("}\n")?;
        for region in &self.memory {
            writeln!(f, "{} = ORIGIN({1}) + LENGTH({1});", region.end, region.name)?;
        }
        Ok(())
    }
}

impl fmt::Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} : ORIGIN = {:#x}, LENGTH = {:#x}", self.name, self.origin, self.length)
    }
}

impl fmt::Display for OutputSection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (start, end) = (self.start, self.end);
        write!(
            f,
            "{} : {{ {start} = {HERE}; {} {end} = {HERE}; }} > {}",
            self.name, self.inputs, self.region
        )?;
        match &self.load {
            Some(load) => write!(f, " AT > {}", load.region),
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
