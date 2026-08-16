use std::io::{self, Read as _, Write as _};

use powershell_formatter::{FormatOptions, format};

fn main() -> io::Result<()> {
    let mut source = String::new();
    io::stdin().read_to_string(&mut source)?;

    let result = format(&source, &FormatOptions::default());

    for diagnostic in result.diagnostics {
        writeln!(io::stderr(), "{}", diagnostic.message)?;
    }

    io::stdout().write_all(result.text.as_bytes())
}
