//! The sets of rules tree-sitter keeps apart, found rather than written: the grammar is generated,
//! and each set the CLI asks for is added, each it calls unnecessary or naming a rule no longer
//! there removed, until it generates with none of these. They are an artifact of tree-sitter's parse tables; which kind wins where
//! several fit is the grammar's (`preferred` in `nodes.vun`).

use std::path::Path;

use vun_data::json::{Json, parse};
use vun_data::{FromData, ToData};
use vun_diag::{Diagnostic, Msg};
use vun_tool::{Arg, Executable};

use super::{FAILED, GENERATE, run};

/// Where the sets are kept, as `cargo gen` names the file `grammar.js` reads.
const FILE: &str = "conflicts.json";
/// How the CLI names a set it needs, and introduces those it does not.
const NEEDED: &str = "Add a conflict for these rules: ";
const UNNECESSARY: &str = "unnecessary conflicts:";
/// How the grammar's loader names a rule a set names that the grammar no longer has.
const UNDEFINED: &str = "Undefined symbol '";
/// Generations tried before giving up.
const ROUNDS: usize = 200;

type Sets = Vec<Vec<String>>;

/// The names between backquotes on `line`: "`a`, `b`".
fn names(line: &str) -> Vec<String> {
    line.split('`').skip(1).step_by(2).map(str::to_owned).collect()
}

/// Generates the parser of `dir`, settling its conflicts first.
pub(super) fn settle(cli: &Executable, dir: &Path) -> Result<(), Box<Diagnostic>> {
    let path = dir.join(FILE);
    let failed = |msg: Msg| Diagnostic::error(FAILED, msg);
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut sets = parse(&text).ok().and_then(|v| Sets::from_data(&v).ok()).unwrap_or_default();
    for _ in 0..ROUNDS {
        let write = |sets: &Sets| {
            std::fs::write(&path, format!("{:#}\n", Json(&sets.to_data())))
                .map_err(|e| Box::new(failed(Msg::new().text("could not write ").code(FILE)).caused_by(&e)))
        };
        write(&sets)?;
        let out = run(cli, dir, vec![Arg::from(GENERATE)])
            .map_err(|f| failed(Msg::new().text("could not run the tree-sitter CLI")).caused_by(&f))?;
        let printed = [out.stdout.as_slice(), out.stderr.as_slice()].concat();
        let printed = String::from_utf8_lossy(&printed);
        // The loader prints its own source too, whose message names no rule: `${propertyName}`.
        let undefined = printed.lines().find_map(|l| {
            let (name, _) = l.split_once(UNDEFINED)?.1.split_once('\'')?;
            name.chars().all(|c| c.is_alphanumeric() || c == '_').then_some(name)
        });
        if let Some(gone) = undefined {
            sets.retain(|s| !s.iter().any(|name| name == gone));
            continue;
        }
        if let Some(needed) = printed.lines().find_map(|l| l.split_once(NEEDED)) {
            let mut set = names(needed.1);
            set.sort();
            if sets.contains(&set) {
                let msg = Msg::new().text("tree-sitter asks again for a conflict it has: ").code(needed.1);
                return Err(Box::new(failed(msg)));
            }
            sets.push(set);
            continue;
        }
        let unnecessary: Sets = printed
            .lines()
            .skip_while(|l| !l.contains(UNNECESSARY))
            .skip(1)
            .map(names)
            .take_while(|n| !n.is_empty())
            .map(|mut n| {
                n.sort();
                n
            })
            .collect();
        if !unnecessary.is_empty() {
            sets.retain(|s| !unnecessary.contains(s));
            continue;
        }
        if let Some(failure) = out.failure() {
            return Err(Box::new(
                failed(Msg::new().text("could not generate the parser")).caused_by(&failure),
            ));
        }
        sets.sort();
        return write(&sets);
    }
    let rounds = u32::try_from(ROUNDS).unwrap_or(u32::MAX);
    Err(Box::new(failed(Msg::new().text("the conflicts did not settle in ").num(rounds).text(" rounds"))))
}
