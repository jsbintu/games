use oracle_sidecar::Sidecar;
use std::io::{self, BufReader};

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();

    let reader = BufReader::new(stdin.lock());
    let writer = stdout.lock();

    let mut sidecar = Sidecar::default();
    sidecar.run_stdio(reader, writer)
}
