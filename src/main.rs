fn main() -> eyre::Result<()> {
    for arg in std::env::args().skip(1) {
        polonius::test_harness(&arg)?;
        polonius::run_dl::run(&arg)?;
    }
    Ok(())
}
